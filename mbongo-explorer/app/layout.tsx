import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Mbongo Explorer",
  description: "Block explorer for Mbongo Chain devnet",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body>
        <header className="header">
          <div className="container header-inner">
            <h1>
              <a href="/">Mbongo Explorer</a>
            </h1>
            <span className="header-tag">devnet</span>
          </div>
        </header>
        <main className="container">{children}</main>
      </body>
    </html>
  );
}
