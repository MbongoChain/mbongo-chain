/**
 * Exact JSON transport for integer values.
 *
 * ## Why this exists
 *
 * `JSON.parse` and `JSON.stringify` are the precision boundary, not the wire.
 * `rpc_v0.2.md` §1 represents `u128` and `u64` as JSON numbers, and a JSON
 * number token is lexically unbounded — the node emits and accepts the exact
 * decimal digits of any `u64`, and of any `u128` on the request path. What
 * loses precision is JavaScript: `JSON.parse` turns every number token into an
 * IEEE-754 double, so `9007199254740993` becomes `9007199254740992`, and
 * `JSON.stringify` refuses `bigint` outright.
 *
 * Both halves of that live inside this client, so both can be replaced without
 * touching the wire. Integers stay JSON numbers; only the parsing and
 * serialisation change. Nothing here alters the RPC contract.
 *
 * ## What it is not
 *
 * Not a general-purpose JSON library, and not exported from the package. It is
 * the transport layer for one client, deliberately kept small enough to read
 * in full.
 *
 * ## Deliberate differences from the native functions
 *
 * - Integer tokens become `bigint`. Tokens with a fraction or exponent stay
 *   `number` — they are not integers, and pretending otherwise would be a lie
 *   about the wire.
 * - Non-finite numbers are **rejected** on serialisation. `JSON.stringify`
 *   silently emits `null` for `NaN` and `Infinity`; for a transport carrying
 *   money that is exactly the class of silent substitution this file exists to
 *   remove.
 * - Number tokens and nesting are bounded, so a hostile response cannot force
 *   unbounded `BigInt` construction or exhaust the stack.
 *
 * Everything else follows RFC 8259 and matches `JSON.parse`: last-wins on
 * duplicate keys, and `__proto__` is defined as an own property rather than
 * assigned through the setter.
 */

/**
 * Longest accepted number token, in characters.
 *
 * `u128::MAX` is 39 digits, so this is generous by any real measure. Its
 * purpose is the pathological case: a million-digit token would otherwise be
 * turned into a `BigInt` before anything could judge it. Grammar validity and
 * semantic range are separate questions — this bound belongs to the parser,
 * and per-field range checks live in `numeric.ts`.
 */
export const MAX_NUMBER_TOKEN_LENGTH = 128;

/** Maximum array/object nesting. Bounds recursion depth on hostile input. */
export const MAX_NESTING_DEPTH = 64;

/** Raised when a JSON document cannot be parsed or serialised exactly. */
export class MbongoJsonError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "MbongoJsonError";
  }
}

// ── Serialisation ────────────────────────────────────────────────────────

/**
 * Serialises a value to JSON text, writing `bigint` as an unquoted canonical
 * integer token.
 *
 * `9007199254740993n` produces `9007199254740993` — not a rounded number, and
 * not a quoted string. `BigInt.prototype.toString` is already canonical
 * base 10: no exponent, no fraction, no leading `+`, no redundant leading
 * zero.
 *
 * String escaping is delegated to `JSON.stringify` on the string itself. Hand
 * rolling escape rules would be a second place for them to be wrong.
 *
 * @throws {MbongoJsonError} on a non-finite number, a cyclic or unsupported
 * value, or nesting past {@link MAX_NESTING_DEPTH}.
 */
export function stringifyExact(value: unknown): string {
  return write(value, 0);
}

function write(value: unknown, depth: number): string {
  if (depth > MAX_NESTING_DEPTH) {
    throw new MbongoJsonError(
      `value nests deeper than ${MAX_NESTING_DEPTH} levels`,
    );
  }

  if (value === null) return "null";

  switch (typeof value) {
    case "boolean":
      return value ? "true" : "false";

    case "bigint":
      // Canonical base-10 digits, the whole point of this module.
      return value.toString();

    case "number":
      if (!Number.isFinite(value)) {
        // JSON has no NaN or Infinity. Native stringify substitutes null;
        // this transport refuses instead.
        throw new MbongoJsonError(
          `cannot serialise the non-finite number ${String(value)}`,
        );
      }
      return JSON.stringify(value);

    case "string":
      return JSON.stringify(value);

    case "object": {
      if (Array.isArray(value)) {
        const items = value.map((item) =>
          item === undefined || typeof item === "function"
            ? "null"
            : write(item, depth + 1),
        );
        return `[${items.join(",")}]`;
      }
      const parts: string[] = [];
      for (const [key, item] of Object.entries(value as object)) {
        if (item === undefined || typeof item === "function") continue;
        parts.push(`${JSON.stringify(key)}:${write(item, depth + 1)}`);
      }
      return `{${parts.join(",")}}`;
    }

    default:
      throw new MbongoJsonError(`cannot serialise a value of type ${typeof value}`);
  }
}

// ── Parsing ──────────────────────────────────────────────────────────────

/**
 * Parses JSON text, returning integer tokens as `bigint` and every other
 * number as `number`.
 *
 * The result carries exact integers but is *not* the public shape: schema
 * normalisation decides which fields stay `bigint` and which become `number`.
 * Leaking `bigint` into `error.code` or a request id would break comparisons
 * that are correct today.
 *
 * @throws {MbongoJsonError} on any input that is not one well-formed JSON
 * document, or that exceeds the parser's bounds.
 */
export function parseExact(text: string): unknown {
  const p = new Parser(text);
  p.skipWhitespace();
  const value = p.parseValue(0);
  p.skipWhitespace();
  if (!p.atEnd()) {
    throw new MbongoJsonError(
      `unexpected trailing content at position ${p.position}`,
    );
  }
  return value;
}

class Parser {
  position = 0;

  constructor(private readonly text: string) {}

  atEnd(): boolean {
    return this.position >= this.text.length;
  }

  private fail(message: string): never {
    throw new MbongoJsonError(`${message} at position ${this.position}`);
  }

  skipWhitespace(): void {
    while (this.position < this.text.length) {
      const c = this.text[this.position];
      if (c === " " || c === "\t" || c === "\n" || c === "\r") this.position++;
      else break;
    }
  }

  parseValue(depth: number): unknown {
    if (depth > MAX_NESTING_DEPTH) {
      this.fail(`nesting deeper than ${MAX_NESTING_DEPTH} levels`);
    }
    if (this.atEnd()) this.fail("unexpected end of input");

    const c = this.text[this.position]!;
    switch (c) {
      case "{":
        return this.parseObject(depth);
      case "[":
        return this.parseArray(depth);
      case '"':
        return this.parseString();
      case "t":
        this.expectLiteral("true");
        return true;
      case "f":
        this.expectLiteral("false");
        return false;
      case "n":
        this.expectLiteral("null");
        return null;
      default:
        // `NaN` and `Infinity` are not JSON, and land here as invalid tokens.
        return this.parseNumber();
    }
  }

  private expectLiteral(literal: string): void {
    if (this.text.startsWith(literal, this.position)) {
      this.position += literal.length;
      return;
    }
    this.fail(`expected ${literal}`);
  }

  private parseObject(depth: number): Record<string, unknown> {
    this.position++; // '{'
    // A null-prototype-free ordinary object, matching JSON.parse's result
    // shape. Keys are defined, never assigned: assigning "__proto__" would
    // reach the prototype setter, while JSON.parse creates an own property.
    const out: Record<string, unknown> = {};

    this.skipWhitespace();
    if (this.text[this.position] === "}") {
      this.position++;
      return out;
    }

    for (;;) {
      this.skipWhitespace();
      if (this.text[this.position] !== '"') this.fail("expected an object key");
      const key = this.parseString();
      this.skipWhitespace();
      if (this.text[this.position] !== ":") this.fail("expected ':'");
      this.position++;
      this.skipWhitespace();
      const value = this.parseValue(depth + 1);

      // Last value wins on a duplicate key, as JSON.parse does.
      Object.defineProperty(out, key, {
        value,
        writable: true,
        enumerable: true,
        configurable: true,
      });

      this.skipWhitespace();
      const c = this.text[this.position];
      if (c === ",") {
        this.position++;
        continue;
      }
      if (c === "}") {
        this.position++;
        return out;
      }
      this.fail("expected ',' or '}'");
    }
  }

  private parseArray(depth: number): unknown[] {
    this.position++; // '['
    const out: unknown[] = [];

    this.skipWhitespace();
    if (this.text[this.position] === "]") {
      this.position++;
      return out;
    }

    for (;;) {
      this.skipWhitespace();
      out.push(this.parseValue(depth + 1));
      this.skipWhitespace();
      const c = this.text[this.position];
      if (c === ",") {
        this.position++;
        continue;
      }
      if (c === "]") {
        this.position++;
        return out;
      }
      this.fail("expected ',' or ']'");
    }
  }

  private parseString(): string {
    this.position++; // opening quote
    let out = "";

    for (;;) {
      if (this.atEnd()) this.fail("unterminated string");
      const c = this.text[this.position]!;

      if (c === '"') {
        this.position++;
        return out;
      }

      if (c === "\\") {
        this.position++;
        if (this.atEnd()) this.fail("unterminated escape");
        const e = this.text[this.position]!;
        this.position++;
        switch (e) {
          case '"': out += '"'; break;
          case "\\": out += "\\"; break;
          case "/": out += "/"; break;
          case "b": out += "\b"; break;
          case "f": out += "\f"; break;
          case "n": out += "\n"; break;
          case "r": out += "\r"; break;
          case "t": out += "\t"; break;
          case "u": {
            const hex = this.text.slice(this.position, this.position + 4);
            if (hex.length !== 4 || !/^[0-9a-fA-F]{4}$/.test(hex)) {
              this.fail("invalid \\u escape");
            }
            out += String.fromCharCode(Number.parseInt(hex, 16));
            this.position += 4;
            break;
          }
          default:
            this.fail(`invalid escape \\${e}`);
        }
        continue;
      }

      // RFC 8259: unescaped control characters are not allowed in strings.
      if (c < " ") this.fail("unescaped control character in string");

      out += c;
      this.position++;
    }
  }

  /**
   * Parses one number token.
   *
   * A token with neither a fraction nor an exponent is an integer and becomes
   * `bigint`; anything else becomes `number`. The grammar is RFC 8259's, so
   * `01`, `+1`, `.5`, `1.`, `NaN` and `Infinity` are all rejected rather than
   * quietly reinterpreted.
   */
  private parseNumber(): bigint | number {
    const start = this.position;

    if (this.text[this.position] === "-") this.position++;

    // int: '0' | [1-9] digit*
    if (this.text[this.position] === "0") {
      this.position++;
    } else if (this.isDigit(this.text[this.position])) {
      while (this.isDigit(this.text[this.position])) this.position++;
    } else {
      this.fail("invalid number");
    }

    let isInteger = true;

    if (this.text[this.position] === ".") {
      isInteger = false;
      this.position++;
      if (!this.isDigit(this.text[this.position])) this.fail("invalid fraction");
      while (this.isDigit(this.text[this.position])) this.position++;
    }

    const e = this.text[this.position];
    if (e === "e" || e === "E") {
      isInteger = false;
      this.position++;
      const sign = this.text[this.position];
      if (sign === "+" || sign === "-") this.position++;
      if (!this.isDigit(this.text[this.position])) this.fail("invalid exponent");
      while (this.isDigit(this.text[this.position])) this.position++;
    }

    const token = this.text.slice(start, this.position);
    if (token.length > MAX_NUMBER_TOKEN_LENGTH) {
      throw new MbongoJsonError(
        `number token of ${token.length} characters exceeds the ` +
          `${MAX_NUMBER_TOKEN_LENGTH}-character limit`,
      );
    }

    // A leading zero followed by more digits ("01") is invalid JSON, and the
    // int rule above already stopped after the '0', so the next character
    // being a digit means the document is malformed.
    if (this.isDigit(this.text[this.position])) this.fail("invalid number");

    return isInteger ? BigInt(token) : Number(token);
  }

  private isDigit(c: string | undefined): boolean {
    return c !== undefined && c >= "0" && c <= "9";
  }
}
