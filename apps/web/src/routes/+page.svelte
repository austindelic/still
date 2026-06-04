<script lang="ts">
	import { resolve } from '$app/paths';

	const shellLines = [
		'$ still init',
		'$ still install ripgrep jq',
		'$ still use node@22',
		'$ still use python@3.12',
		'$ still sync'
	];

	const configLines = [
		'[tools]',
		'node = "22"',
		'python = "3.12"',
		'go = "latest"',
		'',
		'[packages]',
		'latest = ["ripgrep", "fd"]',
		'postgresql = { version = "16" }',
		'',
		'[tasks]',
		'test = "cargo test"',
		'dev = "cargo run"'
	];

	const reasons = [
		{
			title: 'System packages and runtimes belong together',
			body: 'Your project usually needs both. Still treats command line tools, language versions, and local services as one project setup.'
		},
		{
			title: 'Installs should leave a trail',
			body: 'A lockfile gives every machine the same plan, so onboarding starts with the project instead of a long setup note.'
		},
		{
			title: 'Fast should still be readable',
			body: 'Parallel downloads and a shared cache help cut repeat work, while the config stays plain enough to review in a pull request.'
		}
	];

	const features = [
		'Homebrew-style package installs',
		'Project tool versions for Node, Python, Rust, Go, and more',
		'A shared cache to avoid fetching the same work twice',
		'Lockfile-driven setup for repeatable machines',
		'Tasks and services beside the tools that need them',
		'A Rust CLI with a TUI already taking shape'
	];

	const tomlExample = [
		'[tools]',
		'node = "22"',
		'python = "3.12"',
		'rust = "stable"',
		'',
		'[packages]',
		'latest = ["ripgrep", "fd", "jq", "ffmpeg"]',
		'postgresql = { version = "16" }',
		'',
		'[services]',
		'dev-server = { task = "dev" }',
		'',
		'[tasks]',
		'test = "cargo test"',
		'dev = "cargo run"'
	].join('\n');
</script>

<svelte:head>
	<title>Still | Project tools in one place</title>
	<meta
		name="description"
		content="Still is a preview-stage package and toolchain manager for project-local runtimes, system packages, tasks, services, a shared cache, and one readable lockfile."
	/>
</svelte:head>

<main class="min-h-screen overflow-hidden bg-[#282828] text-[#ebdbb2]">
	<section class="border-b border-[#504945] bg-[#282828]">
		<div
			class="mx-auto grid max-w-7xl gap-12 px-5 py-6 sm:px-8 lg:grid-cols-[1fr_0.92fr] lg:px-10 lg:py-8"
		>
			<header class="flex items-center justify-between gap-6 lg:col-span-2">
				<a
					class="flex items-center gap-3 font-semibold"
					href={resolve('/')}
					aria-label="Still home"
				>
					<span
						class="grid size-9 place-items-center rounded-md bg-[#fabd2f] text-lg text-[#1d2021]"
						>S</span
					>
					<span class="text-lg">Still</span>
				</a>
				<nav
					class="flex items-center gap-2 text-sm font-medium text-[#a89984]"
					aria-label="Primary"
				>
					<a
						class="rounded-md px-3 py-2 transition hover:bg-[#3c3836] hover:text-[#ebdbb2]"
						href="https://github.com/austindelic/still"
					>
						GitHub
					</a>
					<a
						class="rounded-md px-3 py-2 transition hover:bg-[#3c3836] hover:text-[#ebdbb2]"
						href="https://github.com/austindelic/still/tree/master/apps/docs"
					>
						Docs
					</a>
				</nav>
			</header>

			<div class="flex flex-col justify-center pt-8 pb-6 lg:min-h-[480px] lg:pt-10 lg:pb-10">
				<p
					class="mb-5 max-w-max rounded-md border border-[#665c54] bg-[#3c3836] px-3 py-1 text-sm font-medium text-[#b8bb26]"
				>
					Preview-stage tooling for people who live in projects
				</p>
				<h1
					class="max-w-4xl text-5xl leading-[0.96] font-semibold tracking-normal text-balance sm:text-7xl lg:text-8xl"
				>
					Still keeps your project tools in one place.
				</h1>
				<p class="mt-7 max-w-2xl text-lg leading-8 text-[#d5c4a1] sm:text-xl">
					Install system packages, pin language runtimes, run project tasks, and keep the whole
					setup tied to a lockfile. One CLI, one cache, one place to see what a project needs.
				</p>
				<div class="mt-9 flex flex-col gap-3 sm:flex-row">
					<a
						class="inline-flex h-12 items-center justify-center rounded-md bg-[#fabd2f] px-5 text-sm font-semibold text-[#1d2021] transition hover:-translate-y-0.5 hover:bg-[#d79921]"
						href="https://github.com/austindelic/still"
					>
						View the repo
					</a>
					<a
						class="inline-flex h-12 items-center justify-center rounded-md border border-[#665c54] bg-[#3c3836] px-5 text-sm font-semibold text-[#ebdbb2] transition hover:-translate-y-0.5 hover:border-[#a89984] hover:bg-[#504945]"
						href="#shape"
					>
						See the shape
					</a>
				</div>
				<p class="mt-5 max-w-xl text-sm leading-6 text-[#a89984]">
					Still is early. The site should invite you in without pretending every planned backend is
					finished.
				</p>
			</div>

			<div class="relative pb-10 lg:flex lg:items-center lg:pb-10">
				<div
					class="w-full rounded-lg border border-[#504945] bg-[#1d2021] p-3 shadow-2xl shadow-black/30"
				>
					<div class="mb-3 flex items-center gap-2 px-2 pt-1">
						<span class="size-3 rounded-full bg-[#fb4934]"></span>
						<span class="size-3 rounded-full bg-[#fabd2f]"></span>
						<span class="size-3 rounded-full bg-[#b8bb26]"></span>
						<span class="ml-2 text-xs font-medium text-[#928374]">still workspace</span>
					</div>
					<div class="grid gap-3 xl:grid-cols-[0.95fr_1.05fr]">
						<div
							class="overflow-x-auto rounded-md bg-[#282828] p-4 font-mono text-xs leading-7 text-[#ebdbb2] sm:text-sm"
						>
							{#each shellLines as line (line)}
								<p class="whitespace-nowrap">
									<span class="text-[#b8bb26]">{line.slice(0, 1)}</span>{line.slice(1)}
								</p>
							{/each}
						</div>
						<div
							class="overflow-x-auto rounded-md bg-[#32302f] p-4 font-mono text-xs leading-6 text-[#d5c4a1] sm:text-sm"
						>
							{#each configLines as line, index (`${index}-${line}`)}
								<p class="whitespace-nowrap" class:min-h-6={line === ''}>{line}</p>
							{/each}
						</div>
					</div>
					<div class="mt-3 grid gap-3 sm:grid-cols-3">
						<div class="rounded-md border border-[#504945] bg-[#282828] p-3">
							<p class="text-xs tracking-[0.16em] text-[#928374] uppercase">Cache</p>
							<p class="mt-1 font-mono text-lg text-[#ebdbb2]">shared</p>
						</div>
						<div class="rounded-md border border-[#504945] bg-[#282828] p-3">
							<p class="text-xs tracking-[0.16em] text-[#928374] uppercase">Plan</p>
							<p class="mt-1 font-mono text-lg text-[#ebdbb2]">locked</p>
						</div>
						<div class="rounded-md border border-[#504945] bg-[#282828] p-3">
							<p class="text-xs tracking-[0.16em] text-[#928374] uppercase">Installs</p>
							<p class="mt-1 font-mono text-lg text-[#ebdbb2]">parallel</p>
						</div>
					</div>
				</div>
			</div>
		</div>
	</section>

	<section class="bg-[#1d2021] py-16 sm:py-20" id="shape">
		<div class="mx-auto grid max-w-7xl gap-12 px-5 sm:px-8 lg:grid-cols-[0.74fr_1fr] lg:px-10">
			<div>
				<p class="text-sm font-semibold tracking-[0.16em] text-[#fb4934] uppercase">
					Why Still exists
				</p>
				<h2 class="mt-4 max-w-xl text-4xl leading-tight font-semibold text-balance sm:text-5xl">
					Project setup should be part of the project.
				</h2>
			</div>
			<div class="grid gap-4">
				{#each reasons as reason (reason.title)}
					<article class="rounded-lg border border-[#504945] bg-[#282828] p-6">
						<h3 class="text-xl font-semibold">{reason.title}</h3>
						<p class="mt-3 leading-7 text-[#d5c4a1]">{reason.body}</p>
					</article>
				{/each}
			</div>
		</div>
	</section>

	<section class="border-y border-[#504945] bg-[#282828] py-20 text-[#ebdbb2] sm:py-24">
		<div class="mx-auto grid max-w-7xl gap-10 px-5 sm:px-8 lg:grid-cols-[1fr_1fr] lg:px-10">
			<div>
				<p class="text-sm font-semibold tracking-[0.16em] text-[#fabd2f] uppercase">
					What it is working toward
				</p>
				<h2 class="mt-4 text-4xl leading-tight font-semibold text-balance sm:text-5xl">
					A small command surface for the messy parts of a dev machine.
				</h2>
				<p class="mt-5 max-w-2xl leading-8 text-[#d5c4a1]">
					Still is built around the idea that setup should be easy to inspect, easy to repeat, and
					boring to run twice.
				</p>
			</div>
			<ul class="grid gap-3 sm:grid-cols-2">
				{#each features as feature (feature)}
					<li
						class="flex min-h-20 items-center rounded-lg border border-[#504945] bg-[#3c3836] px-5 text-base leading-6 text-[#ebdbb2]"
					>
						{feature}
					</li>
				{/each}
			</ul>
		</div>
	</section>

	<section class="bg-[#1d2021] py-20 sm:py-24">
		<div class="mx-auto grid max-w-7xl gap-10 px-5 sm:px-8 lg:grid-cols-[0.9fr_1.1fr] lg:px-10">
			<div>
				<p class="text-sm font-semibold tracking-[0.16em] text-[#b8bb26] uppercase">
					A project file you can read
				</p>
				<h2 class="mt-4 text-4xl leading-tight font-semibold text-balance sm:text-5xl">
					Put the setup beside the code.
				</h2>
				<p class="mt-5 leading-8 text-[#d5c4a1]">
					The config is meant to be plain. A teammate can see which runtimes, packages, tasks, and
					services belong to the project before they install anything.
				</p>
			</div>
			<div class="rounded-lg border border-[#504945] bg-[#282828] p-5 shadow-xl shadow-black/30">
				<pre class="overflow-x-auto text-sm leading-7 text-[#ebdbb2]"><code>{tomlExample}</code
					></pre>
			</div>
		</div>
	</section>

	<section class="bg-[#282828] px-5 py-16 sm:px-8 lg:px-10">
		<div
			class="mx-auto flex max-w-7xl flex-col gap-6 rounded-lg border border-[#504945] bg-[#3c3836] p-6 sm:p-8 lg:flex-row lg:items-center lg:justify-between"
		>
			<div>
				<p class="text-sm font-semibold tracking-[0.16em] text-[#fe8019] uppercase">
					Less setup drift
				</p>
				<h2 class="mt-3 text-3xl leading-tight font-semibold text-balance">
					Bring a project onto a new machine with fewer side notes.
				</h2>
			</div>
			<a
				class="inline-flex h-12 shrink-0 items-center justify-center rounded-md bg-[#fabd2f] px-5 text-sm font-semibold text-[#1d2021] transition hover:-translate-y-0.5 hover:bg-[#d79921]"
				href="https://github.com/austindelic/still"
			>
				Follow the build
			</a>
		</div>
	</section>
</main>
