import type { Metadata } from "next";
import { Rubik, Geist_Mono } from "next/font/google";
import "./globals.css";
import { AppProviders } from "@/components/AppProviders";

const inventivSans = Rubik({
  variable: "--font-rubik",
  subsets: ["latin"],
  weight: ["300", "400", "500", "700", "900"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

/*
 * Note: we keep Geist Mono for code/ids/tables, but we align the main UI font with inventiv-it.fr (Rubik).
 */

/* Legacy (kept disabled)
const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});
*/

export const metadata: Metadata = {
  title: "Inventiv Agents",
  description: "Provision and manage GPU instances",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body
        className={`${inventivSans.variable} ${geistMono.variable} antialiased`}
      >
        <AppProviders>{children}</AppProviders>
      </body>
    </html>
  );
}
