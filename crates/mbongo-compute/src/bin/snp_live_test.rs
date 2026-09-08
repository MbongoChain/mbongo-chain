//! Live AMD SEV-SNP end-to-end test for K-HW. **Runs only inside a SEV-SNP
//! guest** (Linux, `/dev/sev-guest`); anywhere else it prints
//! `RESULT=SKIPPED_NO_HARDWARE` and exits 3, which no CI may count as a
//! pass.
//!
//! In one process, as the guest: it plays the client (seals an input,
//! registers the content key), the control plane (session, lease,
//! challenge), the guest environment (generates an X25519 key **inside this
//! guest**, requests a real report from the platform security processor
//! with `REPORT_DATA` bound to the challenge, obtains the VCEK from the
//! extended report or a supplied file), the verifier (`SnpVerifier` under a
//! strict production policy the operator supplies), J and the release
//! authority, and finally unwraps the content key and decrypts the input —
//! all inside the guest. It prints the `REAL_*` lines the gate requires and
//! writes them to `--out`.
//!
//! Usage (on the guest):
//! ```text
//! snp_live_test --print-report                      # step 1: learn measurement and TCB
//! snp_live_test --product milan --measurement <hex48> \
//!     --min-tcb 2,0,5,68 --out result.txt           # step 2: the gated run
//! ```
//! `--vcek <der>` supplies the VCEK when the extended report carries none
//! (fetch it from AMD KDS for the chip and TCB the report names).

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("RESULT=SKIPPED_NO_HARDWARE");
    println!("snp_live_test runs only inside an AMD SEV-SNP guest (Linux, /dev/sev-guest)");
    std::process::exit(3);
}

#[cfg(target_os = "linux")]
fn main() {
    std::process::exit(linux::run());
}

#[cfg(target_os = "linux")]
mod linux {
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use mbongo_compute::chain::testing::FakeChain;
    use mbongo_compute::chain::ChainClient;
    use mbongo_compute::clock::{Clock, SystemClock};
    use mbongo_compute::confidential::snp::{
        parse_report, report_data_binding, SnpEvidence, SnpPolicy, SnpProduct, SnpVerifier,
        TcbMinimum, SNP_FORMAT, SNP_VERIFIER_KIND,
    };
    use mbongo_compute::confidential::{
        open_input, seal_input, unwrap_key, ContentKey, EnvironmentKey, EvidenceEnvelope,
        KeyReleaseAuthority, MeasurementPolicy, Purpose, ReleaseAuthority, ReleaseRequest,
        TrustPolicy, WrapBinding,
    };
    use mbongo_compute::control_plane::{ControlPlane, ControlPlaneConfig};
    use mbongo_compute::data_plane::{InMemoryDataPlane, LocalKey, Presentation};
    use mbongo_compute::execution::{reference_input_commitment, REVERSE_BYTES_SPEC};
    use mbongo_compute::identity::{ExecutorKey, IdSource, WorkerInstanceId};
    use mbongo_compute::policy::{
        compose, dimension, templates, Advertisement, Claim, DerivedFacts, Scope, SessionFacts,
        Source, TaskFacts, Value,
    };
    use mbongo_core::{
        Address, ComputeTask, Transaction, TransactionPayload, TransactionType,
        COMPUTE_TASK_VERSION,
    };
    use sev::firmware::guest::Firmware;
    use sev::firmware::host::CertType;

    struct Args {
        print_report: bool,
        product: SnpProduct,
        measurement: Option<[u8; 48]>,
        min_tcb: TcbMinimum,
        vcek: Option<Vec<u8>>,
        out: Option<String>,
    }

    fn parse_args() -> Args {
        let mut a = Args {
            print_report: false,
            product: SnpProduct::Milan,
            measurement: None,
            min_tcb: TcbMinimum::default(),
            vcek: None,
            out: None,
        };
        let mut it = std::env::args().skip(1);
        while let Some(k) = it.next() {
            match k.as_str() {
                "--print-report" => a.print_report = true,
                "--product" => {
                    a.product = match it.next().as_deref() {
                        Some("milan") => SnpProduct::Milan,
                        Some("genoa") => SnpProduct::Genoa,
                        Some("turin") => SnpProduct::Turin,
                        other => panic!("unknown product {other:?}"),
                    }
                }
                "--measurement" => {
                    let h = hex::decode(it.next().expect("hex")).expect("hex");
                    let mut m = [0u8; 48];
                    m.copy_from_slice(&h);
                    a.measurement = Some(m);
                }
                "--min-tcb" => {
                    let v: Vec<u8> = it
                        .next()
                        .expect("bl,tee,snp,ucode")
                        .split(',')
                        .map(|x| x.parse().expect("u8"))
                        .collect();
                    a.min_tcb = TcbMinimum {
                        bootloader: v[0],
                        tee: v[1],
                        snp: v[2],
                        microcode: v[3],
                    };
                }
                "--vcek" => {
                    a.vcek = Some(std::fs::read(it.next().expect("path")).expect("vcek file"))
                }
                "--out" => a.out = it.next(),
                other => panic!("unknown argument {other}"),
            }
        }
        a
    }

    fn seed(domain: &str) -> [u8; 32] {
        // Harness-quality randomness for a test run: OS time and pid mixed
        // through BLAKE3. A production guest generates keys from the OS RNG.
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let mut h = blake3::Hasher::new();
        h.update(domain.as_bytes());
        h.update(&t.to_le_bytes());
        h.update(&std::process::id().to_le_bytes());
        *h.finalize().as_bytes()
    }

    #[allow(clippy::too_many_lines)]
    pub fn run() -> i32 {
        let args = parse_args();
        let mut lines: Vec<String> = Vec::new();
        macro_rules! say { ($($t:tt)*) => {{ let s = format!($($t)*); println!("{s}"); lines.push(s); }} }

        let mut fw = match Firmware::open() {
            Ok(f) => f,
            Err(e) => {
                println!("RESULT=SKIPPED_NO_HARDWARE");
                println!("/dev/sev-guest unavailable: {e}");
                return 3;
            }
        };

        // ── control plane, data plane, client (all in-guest for the test)
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let issuer = LocalKey::from_seed(&seed("issuer"));
        let client = LocalKey::from_seed(&seed("client"));
        let executor = ExecutorKey::from_seed(&seed("executor"));
        let mut ids = IdSource::new(seed("ids"));
        let mut cp = ControlPlane::new(
            Arc::clone(&clock),
            IdSource::new(seed("cp")),
            issuer.clone(),
            ControlPlaneConfig::default(),
        );
        let mut dp = InMemoryDataPlane::new(Arc::clone(&clock), IdSource::new(seed("dp")));
        let mut keys = KeyReleaseAuthority::new(IdSource::new(seed("keys")));

        let input = b"live SEV-SNP confidential input".to_vec();
        let task = ComputeTask {
            version: COMPUTE_TASK_VERSION,
            submitter: client.address(),
            executor: executor.address(),
            salt: seed("salt"),
            input_commitment: reference_input_commitment(&input),
            execution_spec: REVERSE_BYTES_SPEC.to_vec(),
        };
        let task_id = task.task_id();
        let content_key = ContentKey::from_bytes(seed("content-key"));
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&seed("seal-nonce")[..24]);
        let sealed = seal_input(
            &content_key,
            nonce,
            &task_id,
            &task.input_commitment,
            &input,
        );
        let object = dp.store_input(
            &client,
            task_id,
            task.input_commitment,
            sealed.to_bytes(),
            3_600,
        );
        dp.register_task(&client, task_id, executor.address()).expect("register");
        dp.delegate_issuer(&client, task_id, issuer.address()).expect("delegate");
        keys.register_content_key(&client, task_id, object, content_key);
        // The control plane learns the task as it always does: from a block.
        let chain = FakeChain::new();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let mut tx = Transaction {
            tx_type: TransactionType::ComputeTask,
            sender: client.address(),
            receiver: Address::zero(),
            amount: 0,
            nonce: 0,
            payload: TransactionPayload::ComputeTask(Box::new(task.clone())),
            signature: [0u8; 64],
        };
        tx.signature = client.sign(&tx.signing_payload());
        rt.block_on(async {
            chain.submit_transaction(&tx).await.expect("chain double");
            chain.produce_block();
            chain.produce_block();
            cp.observe(&chain).await.expect("observe");
        });
        cp.register_confidential_input(task_id, object).expect("confidential input");

        // ── session and lease
        let instance = WorkerInstanceId(ids.next("instance"));
        let ch = cp.session_challenge(executor.address());
        let proof = executor.prove_possession("mbongo:ref-session:v1", &instance.0, &ch);
        let session = cp
            .open_session(
                instance,
                executor.address(),
                vec![REVERSE_BYTES_SPEC.to_vec()],
                ch,
                &proof,
            )
            .expect("session");
        let lease = cp.offer(session.session_id, &mut dp).expect("offer").expect("lease");

        // ── environment key generated inside this guest
        let env_key = EnvironmentKey::from_bytes(seed("environment-key"));

        // ── trust policy: pinned AMD root by digest, operator-supplied
        //    measurement allowlist and TCB minimum; strict platform policy.
        let mut anchors = BTreeMap::new();
        anchors.insert(
            args.product.anchor_id().to_owned(),
            args.product.trust_anchor(),
        );
        let mut allowed = BTreeSet::new();
        if let Some(m) = args.measurement {
            allowed.insert(*blake3::hash(&m).as_bytes());
        }
        let trust = TrustPolicy {
            accepted_formats: [SNP_FORMAT.to_owned()].into_iter().collect(),
            anchors,
            measurement: MeasurementPolicy {
                allowed,
                revoked: BTreeSet::new(),
                min_security_version: 0,
                allow_debug: false,
            },
            max_evidence_age_secs: 300,
            challenge_ttl_secs: 120,
            release_ttl_secs: 120,
        };
        let policy = SnpPolicy::production(args.product, args.min_tcb, 0);
        let mut authority =
            ReleaseAuthority::new(Arc::clone(&clock), IdSource::new(seed("authority")), trust);
        authority.register_verifier(Box::new(SnpVerifier::new(policy)));

        // ── challenge → real report bound to it
        let challenge = authority
            .issue_challenge(
                &session,
                task_id,
                task.executor,
                Purpose::ConfidentialInputRelease,
            )
            .expect("challenge");
        let report_data = report_data_binding(&challenge, &env_key.public());
        let (report, certs) = match fw.get_ext_report(None, Some(report_data), Some(0)) {
            Ok(r) => r,
            Err(e) => {
                println!("RESULT=FAIL");
                println!("platform refused the report request: {e}");
                return 1;
            }
        };
        say!(
            "REAL_QUOTE_GENERATED={}",
            if report.len() == 1184 { "YES" } else { "NO" }
        );
        let parsed = parse_report(&report).expect("report parses");
        say!("REPORT_VERSION={} VMPL={} GUEST_SVN={} POLICY=0x{:x} DEBUG_ALLOWED={} MIGRATION_ALLOWED={} SMT_ENABLED={}",
            parsed.version, parsed.vmpl, parsed.guest_svn, parsed.policy.0, parsed.policy.debug_allowed(), parsed.policy.migrate_ma_allowed(), parsed.plat_info.smt_enabled());
        say!(
            "REPORTED_TCB={},{},{},{} CURRENT_TCB={},{},{},{}",
            parsed.reported_tcb.bootloader,
            parsed.reported_tcb.tee,
            parsed.reported_tcb.snp,
            parsed.reported_tcb.microcode,
            parsed.current_tcb.bootloader,
            parsed.current_tcb.tee,
            parsed.current_tcb.snp,
            parsed.current_tcb.microcode
        );
        say!("MEASUREMENT={}", hex::encode(parsed.measurement));
        say!("CHIP_ID_PREFIX={}", hex::encode(&parsed.chip_id[..16]));
        if args.print_report {
            println!("RESULT=REPORT_PRINTED (pass --measurement and --min-tcb for the gated run)");
            return 0;
        }
        let vcek = args.vcek.clone().or_else(|| {
            certs.as_ref().and_then(|table| {
                table.iter().find(|c| c.cert_type == CertType::VCEK).map(|c| c.data().to_vec())
            })
        });
        let Some(vcek_der) = vcek else {
            println!("RESULT=FAIL");
            println!("no VCEK in the extended report; pass --vcek <der> (fetch from AMD KDS)");
            return 1;
        };
        let evidence = SnpEvidence {
            product: args.product,
            report,
            vcek_der,
            environment_key: env_key.public(),
        };
        let now = clock.now();
        let envelope = EvidenceEnvelope {
            format: SNP_FORMAT.into(),
            verifier_kind: SNP_VERIFIER_KIND.into(),
            challenge_id: challenge.challenge_id,
            session_id: session.session_id,
            executor: session.executor,
            issued_at: now,
            expires_at: now + 120,
            payload: evidence.to_payload(),
        };

        // ── verification
        let verified = match authority.present_evidence(&session, &envelope) {
            Ok(v) => v,
            Err(e) => {
                say!("REAL_QUOTE_VERIFIED=NO ({e})");
                say!("RESULT=FAIL");
                write_out(&args.out, &lines);
                return 1;
            }
        };
        say!("REAL_QUOTE_VERIFIED=YES");
        say!("REAL_CHALLENGE_BOUND=YES");
        say!(
            "REAL_SESSION_BOUND={}",
            if verified.session_id == session.session_id {
                "YES"
            } else {
                "NO"
            }
        );
        say!(
            "REAL_TASK_BOUND={}",
            if verified.task_id == task_id {
                "YES"
            } else {
                "NO"
            }
        );
        say!(
            "REAL_EXECUTOR_BOUND={}",
            if verified.executor == task.executor {
                "YES"
            } else {
                "NO"
            }
        );
        say!(
            "REAL_ENVIRONMENT_KEY_BOUND={}",
            if verified.environment_key == env_key.public() {
                "YES"
            } else {
                "NO"
            }
        );
        say!("REAL_MEASUREMENT_CHECKED=YES");
        say!("REAL_SECURITY_STATE_CHECKED=YES");
        say!("SECURITY_LABEL={}", verified.security_label);

        // ── J and release
        let mut claims = BTreeMap::new();
        claims.insert(
            dimension::CONTRACT_VERSION.to_owned(),
            Claim::plain(Value::Token("provider-policy-v1".into()), Scope::Session),
        );
        claims.insert(
            dimension::REPRESENTATION_TAGS.to_owned(),
            Claim::plain(
                Value::Set(["mbongo-ref:bytes:v1".to_owned()].into_iter().collect()),
                Scope::Session,
            ),
        );
        claims.insert(
            dimension::PROFILE_TAGS.to_owned(),
            Claim::plain(
                Value::Set(
                    [String::from_utf8_lossy(REVERSE_BYTES_SPEC).into_owned()]
                        .into_iter()
                        .collect(),
                ),
                Scope::Session,
            ),
        );
        claims.insert(
            dimension::CONFIDENTIAL_EXECUTION.to_owned(),
            Claim::plain(Value::Bool(true), Scope::Session),
        );
        let facts = SessionFacts {
            session_id: session.session_id,
            proved_executor: session.executor,
            worker_class: None,
            claims,
        };
        let task_facts = TaskFacts {
            executor: task.executor,
            profile_tag: String::from_utf8_lossy(&task.execution_spec).into_owned(),
            representation_tag: "mbongo-ref:bytes:v1".into(),
        };
        let advertisement = Advertisement {
            provider_id: "live".into(),
            executors: vec![task.executor],
            issued_at: 0,
            expires_at: u64::MAX,
            revoked: false,
            claims: BTreeMap::new(),
            worker_classes: BTreeMap::new(),
        };
        let policy = compose(&[templates::confidential(Source::DataOwner)]).expect("template");
        let (auth, decision) = match authority.authorize_release(&ReleaseRequest {
            session: &session,
            challenge_id: challenge.challenge_id,
            task: &task_facts,
            session_facts: &facts,
            advertisement: &advertisement,
            derived: &DerivedFacts::default(),
            policy: &policy,
        }) {
            Ok(x) => x,
            Err(e) => {
                say!("REAL_J_POLICY_PASS=NO ({e})");
                say!("RESULT=FAIL");
                write_out(&args.out, &lines);
                return 1;
            }
        };
        say!(
            "REAL_J_POLICY_PASS={}",
            if decision.eligible { "YES" } else { "NO" }
        );
        let (cap, grant) = cp
            .authorize_fetch_confidential(
                session.session_id,
                lease.lease_id,
                auth.release_id,
                &mut authority,
                &mut dp,
            )
            .expect("confidential fetch");
        let wrapped = keys.release(&grant, object).expect("key release");
        say!("REAL_KEY_RELEASE=YES");

        // ── decrypt inside the guest
        let dch = dp.issue_challenge(executor.address());
        let p = Presentation::sign(cap, dch, &executor);
        let sealed_bytes = dp.fetch_input(&p).expect("fetch sealed");
        let sealed = mbongo_compute::confidential::SealedInput::from_bytes(sealed_bytes.as_bytes())
            .expect("sealed");
        let binding = WrapBinding {
            release_id: grant.release_id,
            task_id: grant.task_id,
            executor: grant.executor,
            session_id: grant.session_id,
        };
        let key = unwrap_key(&wrapped, &env_key, &binding).expect("unwrap inside the guest");
        let plain = open_input(&key, &task_id, &task.input_commitment, &sealed).expect("open");
        let ok = plain.as_bytes() == input.as_slice()
            && reference_input_commitment(plain.as_bytes()) == task.input_commitment;
        say!(
            "REAL_PRIVATE_INPUT_DECRYPTED_INSIDE_PROTECTED_ENV={}",
            if ok { "YES" } else { "NO" }
        );
        say!("RESULT={}", if ok { "PASS_HARDWARE" } else { "FAIL" });
        write_out(&args.out, &lines);
        i32::from(!ok)
    }

    fn write_out(out: &Option<String>, lines: &[String]) {
        if let Some(p) = out {
            std::fs::write(p, lines.join("\n") + "\n").expect("write results");
        }
    }
}
