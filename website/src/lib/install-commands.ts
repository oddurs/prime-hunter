export type OS = "macos" | "linux" | "windows";

export interface InstallMethod {
  label: string;
  commands: string;
}

export const installMethods: Record<OS, InstallMethod[]> = {
  macos: [
    {
      label: "Build from source",
      commands: `# Install dependencies
brew install gmp rust

# Clone and build
git clone https://github.com/darkreach/darkreach.git
cd darkreach
cargo build --release

# Verify
./target/release/darkreach --version`,
    },
    {
      label: "Docker",
      commands: `# Pull and run
docker pull darkreach/darkreach:latest
docker run --rm darkreach/darkreach --version`,
    },
  ],
  linux: [
    {
      label: "Build from source",
      commands: `# Install dependencies (Debian/Ubuntu)
sudo apt install build-essential libgmp-dev m4
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/darkreach/darkreach.git
cd darkreach
cargo build --release

# Verify
./target/release/darkreach --version`,
    },
    {
      label: "Docker",
      commands: `# Pull and run
docker pull darkreach/darkreach:latest
docker run --rm darkreach/darkreach --version`,
    },
  ],
  windows: [
    {
      label: "WSL2 (recommended)",
      commands: `# Inside WSL2 (Ubuntu)
sudo apt install build-essential libgmp-dev m4
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

git clone https://github.com/darkreach/darkreach.git
cd darkreach
cargo build --release`,
    },
    {
      label: "Docker Desktop",
      commands: `# Pull and run
docker pull darkreach/darkreach:latest
docker run --rm darkreach/darkreach --version`,
    },
  ],
};

export const systemRequirements = [
  { component: "CPU", minimum: "4 cores", recommended: "8+ cores (AVX2)" },
  { component: "RAM", minimum: "4 GB", recommended: "16+ GB" },
  { component: "Disk", minimum: "1 GB", recommended: "10+ GB" },
  { component: "OS", minimum: "Linux / macOS", recommended: "Ubuntu 22.04+ / macOS 13+" },
  { component: "Rust", minimum: "1.75+", recommended: "Latest stable" },
  { component: "GMP", minimum: "6.2+", recommended: "6.3+" },
];
