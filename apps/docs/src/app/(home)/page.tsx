import Link from "next/link";

export default function HomePage() {
  return (
    <div className="mx-auto flex w-full max-w-5xl flex-1 flex-col justify-center px-6 py-16">
      <div className="max-w-3xl">
        <p className="mb-3 text-sm font-medium uppercase text-fd-muted-foreground">
          Still docs
        </p>
        <h1 className="mb-5 text-4xl font-semibold text-fd-foreground md:text-6xl">
          One project file for the tools your work depends on.
        </h1>
        <p className="mb-8 max-w-2xl text-lg leading-8 text-fd-muted-foreground">
          Still is a Rust package and toolchain manager for projects that need
          repeatable setup without a pile of shell notes. The docs are written
          for builders: clear examples, honest status notes, and no filler.
        </p>
        <div className="flex flex-wrap gap-3">
          <Link
            href="/docs"
            className="rounded-md bg-fd-primary px-4 py-2.5 text-sm font-medium text-fd-primary-foreground"
          >
            Read the docs
          </Link>
          <Link
            href="/docs/getting-started"
            className="rounded-md border px-4 py-2.5 text-sm font-medium"
          >
            Get started
          </Link>
        </div>
      </div>
    </div>
  );
}
