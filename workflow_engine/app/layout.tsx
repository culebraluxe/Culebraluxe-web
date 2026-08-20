import type { Metadata } from 'next';
import Link from 'next/link';
import './globals.css';

export const metadata: Metadata = {
  title: 'Workflow Engine',
  description: 'Lightweight process engine',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="bg-gray-100 text-gray-900 min-h-screen">
        <header className="bg-white border-b shadow-sm">
          <div className="max-w-6xl mx-auto px-4 h-14 flex items-center justify-between">
            <div className="flex items-center gap-8">
              <Link href="/dashboard" className="font-bold text-lg">
                Workflow
              </Link>
              <nav className="hidden sm:flex gap-6 text-sm">
                <Link href="/dashboard" className="hover:text-blue-600">
                  Dashboard
                </Link>
                <Link href="/processes/start" className="hover:text-blue-600">
                  Start Process
                </Link>
                <Link href="/tasks" className="hover:text-blue-600">
                  My Tasks
                </Link>
              </nav>
            </div>
            <div className="text-sm text-gray-600">john.doe</div>
          </div>
        </header>
        <main>{children}</main>
      </body>
    </html>
  );
}
