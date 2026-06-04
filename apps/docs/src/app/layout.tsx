import { RootProvider } from "fumadocs-ui/provider/next";
import type { Metadata } from "next";
import "./global.css";

export const metadata: Metadata = {
  metadataBase: new URL("https://still.dev"),
  title: {
    default: "Still",
    template: "%s | Still",
  },
  description:
    "Still is a config-first project environment manager for tools, packages, apps, tasks, services, and agent setup.",
};

export default function Layout({ children }: LayoutProps<"/">) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body className="flex flex-col min-h-screen">
        <RootProvider>{children}</RootProvider>
      </body>
    </html>
  );
}
