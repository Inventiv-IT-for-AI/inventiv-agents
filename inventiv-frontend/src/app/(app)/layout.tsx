import { Sidebar } from "@/components/Sidebar";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";

export default async function AppLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  const sessionCookie = process.env.SESSION_COOKIE_NAME ?? "inventiv_session";
  // Next 16 can expose `cookies()` as an async function (returns a Promise).
  // If we don't await it, `.get()` / `.toString()` will fail at runtime.
  const cookieStore = await cookies();
  const token = cookieStore.get(sessionCookie)?.value;
  if (!token) {
    // Keep it simple & safe: protect the whole (app) route group.
    // (We previously did this in `src/middleware.ts`, which is now deprecated by Next.)
    redirect("/login");
  }

  return (
    <div className="flex h-screen overflow-hidden">
      <Sidebar />
      <main className="flex-1 overflow-y-auto bg-background">{children}</main>
    </div>
  );
}


