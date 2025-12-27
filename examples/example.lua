return {
	package = {
		name = "bun",
		version = "1.3.5",
		description = "Fast all-in-one JavaScript runtime",
		homepage = "https://bun.sh",
		license = "MIT",
	},

	dependencies = {
		build = {},
		runtime = {},
	},

	install = {
		bin = { "bun" },

		macos = {
			aarch64 = {
				url = "https://example.com/bun-macos-aarch64.zip",
				sha256 = "abc123...",
				archive = "zip",
				strip = 1,
			},
			x86_64 = {
				url = "https://example.com/bun-macos-x86_64.zip",
				sha256 = "def456...",
				archive = "zip",
				strip = 1,
			},
		},

		linux = {
			aarch64 = {
				url = "https://example.com/bun-linux-aarch64.zip",
				sha256 = "ghi789...",
				archive = "zip",
				strip = 1,
			},
			x86_64 = {
				url = "https://example.com/bun-linux-x86_64.zip",
				sha256 = "jkl012...",
				archive = "zip",
				strip = 1,
			},
		},

		windows = {
			x86_64 = {
				url = "https://example.com/bun-windows-x86_64.zip",
				sha256 = "mno345...",
				archive = "zip",
				strip = 1,
			},
		},
	},

	uninstall = {
		macos = { mode = "remove-installed-files" },
		linux = { mode = "remove-installed-files" },
		windows = { mode = "remove-installed-files" },
	},

	test = {
		command = { "bun", "--version" },
	},
}
