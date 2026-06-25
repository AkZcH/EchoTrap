// src/persona.rs
// Persona enum lives here so both config.rs and personas.rs can import it
// without a circular dependency.

/// Available protocol personas. Selected via --persona CLI flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Persona {
    /// OpenSSH 8.9p1 banner — most common on Ubuntu 22.04 servers
    Ssh,
    /// nginx/1.18.0 HTTP server — responds to any request with 200 OK
    Http,
    /// Redis 7.x — PONG on connect, ERR on commands
    Redis,
    /// Raw echo — original behavior, useful for testing
    Raw,
}

impl Persona {
    pub fn banner(self) -> &'static [u8] {
        match self {
            Persona::Ssh => b"SSH-2.0-OpenSSH_8.9p1 Ubuntu-3ubuntu0.6\r\n",
            Persona::Http => b"",
            Persona::Redis => b"+PONG\r\n",
            Persona::Raw => b"Welcome to EchoTrap Service v1.2\r\n",
        }
    }

    pub fn banner_str(self) -> &'static str {
        match self {
            Persona::Ssh => "SSH-2.0-OpenSSH_8.9p1 Ubuntu-3ubuntu0.6\r\n",
            Persona::Http => "",
            Persona::Redis => "+PONG\r\n",
            Persona::Raw => "Welcome to EchoTrap Service v1.2\r\n",
        }
    }

    pub fn jitter_ms(self) -> (u64, u64) {
        match self {
            Persona::Ssh => (20, 150),
            Persona::Http => (5, 80),
            Persona::Redis => (1, 10),
            Persona::Raw => (0, 5),
        }
    }
}

impl std::fmt::Display for Persona {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Persona::Ssh => write!(f, "ssh"),
            Persona::Http => write!(f, "http"),
            Persona::Redis => write!(f, "redis"),
            Persona::Raw => write!(f, "raw"),
        }
    }
}
