import {
  ArrowRight,
  Bot,
  Braces,
  Check,
  ChevronRight,
  ClipboardCheck,
  FileCog,
  GitBranch,
  ListChecks,
  Monitor,
  Package,
  Play,
  ShieldCheck,
  SquareTerminal,
  Terminal,
  Workflow,
} from "lucide-react";
import Image from "next/image";
import Link from "next/link";
import type { ComponentType } from "react";

const palette = [
  ["bg-[#fb4934]", "red"],
  ["bg-[#fe8019]", "orange"],
  ["bg-[#fabd2f]", "yellow"],
  ["bg-[#b8bb26]", "green"],
  ["bg-[#8ec07c]", "aqua"],
  ["bg-[#83a598]", "blue"],
  ["bg-[#d3869b]", "purple"],
];

const capabilities = [
  {
    icon: Terminal,
    title: "Tools",
    text: "Runtimes, toolchains, and developer CLIs live in committed state instead of onboarding notes.",
  },
  {
    icon: Package,
    title: "Packages",
    text: "System libraries and command packages resolve through platform-aware names and backends.",
  },
  {
    icon: Monitor,
    title: "Apps",
    text: "Desktop dependencies are modeled separately from packages because install behavior differs.",
  },
  {
    icon: ListChecks,
    title: "Tasks",
    text: "Named commands and dependency graphs run with the same managed env every time.",
  },
  {
    icon: Bot,
    title: "Agents",
    text: "Shared instructions and skills become reproducible project setup, not local folklore.",
  },
  {
    icon: ShieldCheck,
    title: "Trust",
    text: "Project-defined executable behavior is gated behind explicit review and stale-config checks.",
  },
];

const docsRoutes = [
  {
    icon: Play,
    title: "Getting started",
    text: "Create a config, install dependencies, sync state, run commands, and activate a shell.",
    href: "/docs/getting-started",
  },
  {
    icon: SquareTerminal,
    title: "Commands",
    text: "Every top-level command, scope flag, install grouping rule, and diagnostic surface.",
    href: "/docs/commands",
  },
  {
    icon: FileCog,
    title: "Configuration",
    text: "Tools, env, packages, apps, tasks, services, agents, filters, and write rules.",
    href: "/docs/configuration",
  },
  {
    icon: GitBranch,
    title: "Platforms",
    text: "Multi-OS names, backend selection, default resolution, and lockfile strategy.",
    href: "/docs/platforms-and-backends",
  },
];

export default function HomePage() {
  return (
    <main className="gb-grain min-h-dvh overflow-hidden bg-[#1d2021] text-[#fbf1c7]">
      <section className="gb-lines relative isolate border-[#504945] border-b">
        <div className="absolute inset-0 -z-10 bg-[radial-gradient(circle_at_78%_10%,rgba(250,189,47,0.14),transparent_28%),radial-gradient(circle_at_18%_22%,rgba(131,165,152,0.12),transparent_34%),linear-gradient(180deg,#282828_0%,#1d2021_74%)]" />
        <div className="mx-auto grid min-h-[calc(100dvh-64px)] max-w-7xl items-center gap-12 px-5 py-14 md:px-8 lg:grid-cols-[0.92fr_1.08fr] lg:px-10">
          <div>
            <div className="mb-7 inline-flex items-center gap-3 border border-[#665c54] bg-[#282828]/85 px-3 py-2 text-[#d5c4a1] text-sm shadow-[0_18px_80px_rgba(0,0,0,0.28)] backdrop-blur">
              <Image
                src="/brand/still-mark.svg"
                width={28}
                height={28}
                alt=""
                className="h-7 w-7"
              />
              Config-first project environments
            </div>

            <h1 className="max-w-3xl text-balance font-semibold text-6xl tracking-normal text-[#fbf1c7] md:text-8xl">
              Still keeps the project warm.
            </h1>
            <p className="mt-7 max-w-2xl text-pretty text-xl text-[#d5c4a1] leading-9">
              One committed `still.toml` for tools, packages, apps, env,
              services, tasks, and agent setup. Rebuild the working environment
              without rebuilding tribal memory.
            </p>

            <div className="mt-9 flex flex-wrap gap-3">
              <Link
                href="/docs/getting-started"
                className="group inline-flex h-12 items-center gap-2 bg-[#fabd2f] px-5 font-semibold text-[#1d2021] text-sm shadow-[0_18px_60px_rgba(250,189,47,0.24)] transition hover:bg-[#fe8019]"
              >
                Start building
                <ArrowRight
                  className="h-4 w-4 transition group-hover:translate-x-0.5"
                  aria-hidden
                />
              </Link>
              <Link
                href="/docs/configuration"
                className="inline-flex h-12 items-center gap-2 border border-[#665c54] bg-[#282828]/80 px-5 font-semibold text-[#fbf1c7] text-sm transition hover:border-[#fabd2f] hover:bg-[#32302f]"
              >
                <FileCog className="h-4 w-4" aria-hidden />
                Read the config
              </Link>
            </div>

            <div className="mt-11 grid max-w-2xl grid-cols-3 border border-[#504945] bg-[#282828]/70">
              <Metric value="7" label="sections" />
              <Metric value="3" label="platforms" />
              <Metric value="1" label="lockfile" />
            </div>
          </div>

          <HeroConsole />
        </div>
      </section>

      <section className="border-[#504945] border-b bg-[#282828]">
        <div className="mx-auto grid max-w-7xl gap-6 px-5 py-8 md:grid-cols-[0.7fr_1.3fr] md:px-8 lg:px-10">
          <div>
            <p className="flex items-center gap-2 font-semibold text-[#fabd2f] text-sm">
              <Workflow className="h-4 w-4" aria-hidden />
              From empty project to active environment
            </p>
            <h2 className="mt-3 max-w-md text-balance font-semibold text-3xl text-[#fbf1c7]">
              A workflow short enough to remember, strict enough to trust.
            </h2>
          </div>

          <div className="grid gap-3 lg:grid-cols-4">
            <FlowStep
              command="still init"
              detail="write config"
              tone="#fabd2f"
            />
            <FlowStep
              command="still install"
              detail="add state"
              tone="#b8bb26"
            />
            <FlowStep
              command="still sync"
              detail="resolve host"
              tone="#83a598"
            />
            <FlowStep
              command="still task"
              detail="run context"
              tone="#d3869b"
            />
          </div>
        </div>
      </section>

      <section className="bg-[#fbf1c7] text-[#282828]">
        <div className="mx-auto max-w-7xl px-5 py-20 md:px-8 lg:px-10">
          <div className="grid gap-8 lg:grid-cols-[0.9fr_1.1fr] lg:items-end">
            <div>
              <p className="mb-4 font-semibold text-[#98971a] text-sm">
                The whole environment, not just one package manager
              </p>
              <h2 className="max-w-2xl text-balance font-semibold text-4xl tracking-normal md:text-6xl">
                The docs now explain the system end to end.
              </h2>
            </div>
            <p className="max-w-xl text-[#504945] text-lg leading-8 lg:justify-self-end">
              Still's docs map to the CLI spec, example config, and JSON schema:
              install semantics, config mutation, trust gates, services, task
              graphs, agent skills, platform filters, and lockfile behavior.
            </p>
          </div>

          <div className="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {capabilities.map((item) => (
              <Capability key={item.title} {...item} />
            ))}
          </div>
        </div>
      </section>

      <section className="border-[#504945] border-y bg-[#1d2021] text-[#fbf1c7]">
        <div className="mx-auto grid max-w-7xl gap-10 px-5 py-20 md:px-8 lg:grid-cols-[0.9fr_1.1fr] lg:px-10">
          <div>
            <p className="mb-4 flex items-center gap-2 font-semibold text-[#fe8019] text-sm">
              <ShieldCheck className="h-4 w-4" aria-hidden />
              Trust is part of the interface
            </p>
            <h2 className="max-w-xl text-balance font-semibold text-4xl tracking-normal md:text-5xl">
              Read config freely. Execute project behavior deliberately.
            </h2>
            <p className="mt-5 max-w-xl text-[#d5c4a1] leading-7">
              Still separates dependency resolution from project-defined code.
              Tasks, services, env files, and external agent skills are reviewed
              through the same trust model before they run.
            </p>
            <Link
              href="/docs/trust-and-safety"
              className="mt-8 inline-flex h-11 items-center gap-2 border border-[#665c54] px-4 font-semibold text-sm transition hover:border-[#fabd2f] hover:bg-[#282828]"
            >
              Trust model
              <ChevronRight className="h-4 w-4" aria-hidden />
            </Link>
          </div>

          <div className="grid gap-3">
            <TrustRow
              label="Read still.toml"
              value="always available"
              tone="#b8bb26"
            />
            <TrustRow
              label="Resolve public dependencies"
              value="backend-owned"
              tone="#83a598"
            />
            <TrustRow
              label="Run project tasks"
              value="requires trust"
              tone="#fe8019"
            />
            <TrustRow
              label="Load env files"
              value="requires trust"
              tone="#fb4934"
            />
            <TrustRow
              label="Sync external skills"
              value="requires trust"
              tone="#d3869b"
            />
          </div>
        </div>
      </section>

      <section className="bg-[#f2e5bc] text-[#282828]">
        <div className="mx-auto max-w-7xl px-5 py-20 md:px-8 lg:px-10">
          <div className="flex flex-col gap-5 md:flex-row md:items-end md:justify-between">
            <div>
              <p className="mb-4 flex items-center gap-2 font-semibold text-[#79740e] text-sm">
                <ClipboardCheck className="h-4 w-4" aria-hidden />
                Read next
              </p>
              <h2 className="max-w-3xl text-balance font-semibold text-4xl tracking-normal md:text-5xl">
                A first-class reference for a config-first CLI.
              </h2>
            </div>
            <Link
              href="/docs"
              className="inline-flex h-11 items-center gap-2 border border-[#7c6f64] bg-[#fbf1c7] px-4 font-semibold text-sm transition hover:border-[#282828] hover:bg-[#282828] hover:text-[#fbf1c7]"
            >
              Full docs
              <ArrowRight className="h-4 w-4" aria-hidden />
            </Link>
          </div>

          <div className="mt-10 grid gap-4 md:grid-cols-2 xl:grid-cols-4">
            {docsRoutes.map((route) => (
              <DocRoute key={route.href} {...route} />
            ))}
          </div>
        </div>
      </section>
    </main>
  );
}

function HeroConsole() {
  return (
    <div className="relative">
      <div className="absolute -inset-6 -z-10 border border-[#504945]/60 bg-[#282828]/35 shadow-[0_34px_120px_rgba(0,0,0,0.42)]" />
      <div className="border border-[#665c54] bg-[#1d2021] shadow-[0_24px_90px_rgba(0,0,0,0.36)]">
        <div className="flex items-center justify-between border-[#504945] border-b bg-[#282828] px-4 py-3">
          <div className="flex items-center gap-2 text-[#d5c4a1] text-sm">
            <SquareTerminal className="h-4 w-4 text-[#fabd2f]" aria-hidden />
            still.toml
          </div>
          <div className="flex gap-2" aria-hidden>
            {palette.slice(0, 3).map(([className, name]) => (
              <span key={name} className={`h-2.5 w-2.5 ${className}`} />
            ))}
          </div>
        </div>
        <div className="grid lg:grid-cols-[1.08fr_0.92fr]">
          <pre className="overflow-x-auto border-[#504945] border-b p-5 font-mono text-[13px] text-[#d5c4a1] leading-7 lg:border-r lg:border-b-0">
            <code>
              <span className="text-[#83a598]">[tools]</span>
              {"\n"}
              <span className="text-[#b8bb26]">node</span> ={" "}
              <span className="text-[#fabd2f]">"22"</span>
              {"\n"}
              <span className="text-[#b8bb26]">rust</span> ={" "}
              <span className="text-[#fabd2f]">"stable"</span>
              {"\n\n"}
              <span className="text-[#83a598]">[packages]</span>
              {"\n"}
              <span className="text-[#b8bb26]">latest</span> = [
              <span className="text-[#fabd2f]">"ripgrep"</span>,{" "}
              <span className="text-[#fabd2f]">"jq"</span>]{"\n\n"}
              <span className="text-[#83a598]">[tasks.ci]</span>
              {"\n"}
              <span className="text-[#b8bb26]">depends</span> = [
              <span className="text-[#fabd2f]">"lint"</span>,{" "}
              <span className="text-[#fabd2f]">"test"</span>]{"\n"}
              <span className="text-[#b8bb26]">run</span> ={" "}
              <span className="text-[#fabd2f]">"echo CI passed"</span>
            </code>
          </pre>
          <div className="p-5">
            <div className="mb-4 flex items-center gap-2 text-[#fabd2f] text-sm">
              <Braces className="h-4 w-4" aria-hidden />
              resolved plan
            </div>
            <div className="space-y-3">
              <PlanLine title="tool rust@stable" color="#b8bb26" />
              <PlanLine title="package ripgrep@latest" color="#83a598" />
              <PlanLine title="task graph ci" color="#d3869b" />
              <PlanLine title="trust marker fresh" color="#fabd2f" />
            </div>
            <div className="mt-6 border border-[#504945] bg-[#282828] p-4">
              <div className="mb-3 text-[#d5c4a1] text-xs uppercase tracking-[0.18em]">
                next
              </div>
              <div className="font-mono text-[#fbf1c7] text-sm">
                $ still sync
              </div>
              <div className="mt-2 text-[#a89984] text-sm">
                lockfile refreshed, missing state installed
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function PlanLine({ title, color }: { title: string; color: string }) {
  return (
    <div className="flex items-center justify-between border border-[#504945] bg-[#282828] px-3 py-2">
      <div className="flex items-center gap-2 text-[#d5c4a1] text-sm">
        <span className="h-2.5 w-2.5" style={{ backgroundColor: color }} />
        {title}
      </div>
      <Check className="h-4 w-4 text-[#b8bb26]" aria-hidden />
    </div>
  );
}

function Metric({ value, label }: { value: string; label: string }) {
  return (
    <div className="border-[#504945] border-r px-4 py-4 last:border-r-0">
      <div className="font-semibold text-2xl text-[#fabd2f]">{value}</div>
      <div className="mt-1 text-[#a89984] text-[11px] uppercase tracking-[0.18em]">
        {label}
      </div>
    </div>
  );
}

function FlowStep({
  command,
  detail,
  tone,
}: {
  command: string;
  detail: string;
  tone: string;
}) {
  return (
    <div className="border border-[#504945] bg-[#1d2021] p-4 transition hover:-translate-y-0.5 hover:border-[#fabd2f]">
      <div
        className="flex items-center gap-2 font-mono text-sm"
        style={{ color: tone }}
      >
        <Check className="h-4 w-4" aria-hidden />
        {command}
      </div>
      <p className="mt-2 text-[#a89984] text-sm">{detail}</p>
    </div>
  );
}

function Capability({
  icon: Icon,
  title,
  text,
}: {
  icon: ComponentType<{ className?: string; "aria-hidden"?: boolean }>;
  title: string;
  text: string;
}) {
  return (
    <div className="group min-h-44 border border-[#d5c4a1] bg-[#f9f5d7] p-5 shadow-[0_18px_50px_rgba(60,56,54,0.08)] transition hover:-translate-y-0.5 hover:border-[#79740e] hover:bg-[#fbf1c7]">
      <div className="flex h-11 w-11 items-center justify-center border border-[#d5c4a1] bg-[#282828] text-[#fabd2f] transition group-hover:bg-[#79740e] group-hover:text-[#fbf1c7]">
        <Icon className="h-5 w-5" aria-hidden />
      </div>
      <h3 className="mt-5 font-semibold text-lg">{title}</h3>
      <p className="mt-3 text-[#665c54] text-sm leading-6">{text}</p>
    </div>
  );
}

function TrustRow({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone: string;
}) {
  return (
    <div className="grid gap-3 border border-[#504945] bg-[#282828] p-4 sm:grid-cols-[1fr_auto] sm:items-center">
      <div className="flex items-center gap-3">
        <span className="h-3 w-3" style={{ backgroundColor: tone }} />
        <span className="font-medium">{label}</span>
      </div>
      <span className="font-mono text-[#d5c4a1] text-sm">{value}</span>
    </div>
  );
}

function DocRoute({
  icon: Icon,
  title,
  text,
  href,
}: {
  icon: ComponentType<{ className?: string; "aria-hidden"?: boolean }>;
  title: string;
  text: string;
  href: string;
}) {
  return (
    <Link
      href={href}
      className="group flex min-h-56 flex-col justify-between border border-[#d5c4a1] bg-[#fbf1c7] p-5 transition hover:-translate-y-0.5 hover:border-[#282828] hover:bg-[#f9f5d7] hover:shadow-[0_24px_70px_rgba(60,56,54,0.14)]"
    >
      <div>
        <div className="mb-6 flex items-center justify-between">
          <div className="flex h-10 w-10 items-center justify-center border border-[#d5c4a1] bg-[#282828] text-[#fabd2f]">
            <Icon className="h-5 w-5" aria-hidden />
          </div>
          <ArrowRight
            className="h-4 w-4 text-[#7c6f64] transition group-hover:translate-x-1 group-hover:text-[#282828]"
            aria-hidden
          />
        </div>
        <h3 className="font-semibold text-lg">{title}</h3>
        <p className="mt-3 text-[#665c54] text-sm leading-6">{text}</p>
      </div>
    </Link>
  );
}
