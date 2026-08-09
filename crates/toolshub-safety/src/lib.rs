//! `toolshub-safety` — command safety analysis engine.
//!
//! Provides a [`SafetyChecker`] that analyses shell command strings for
//! dangerous patterns and returns structured [`SafetyReport`]s.
//!
//! # Design
//!
//! The engine works entirely offline — no AI or network call is required.
//! Danger patterns are compiled at startup into a static rule table.
//! The checker is designed to be fast enough to run on every keystroke
//! in an interactive shell integration.

use serde::{Deserialize, Serialize};
use toolshub_core::types::DangerLevel;

// ── Rule table ────────────────────────────────────────────────────────────────

/// A single safety rule that matches a command pattern.
#[derive(Debug, Clone)]
pub struct SafetyRule {
    /// The substring or pattern to search for in the command.
    pub pattern: &'static str,
    /// The danger classification.
    pub level: DangerLevel,
    /// Short label for the type of risk.
    pub label: &'static str,
    /// Human-readable explanation of what this command does.
    pub explanation: &'static str,
    /// What could go wrong.
    pub consequence: &'static str,
    /// A safer alternative, if applicable.
    pub alternative: Option<&'static str>,
}

/// All built-in safety rules, ordered from most to least dangerous.
pub static RULES: &[SafetyRule] = &[
    SafetyRule {
        pattern: ":(){:|:&};:",
        level: DangerLevel::Critical,
        label: "Fork Bomb",
        explanation: "Creates a function that spawns itself recursively, exhausting all process slots.",
        consequence: "Immediate system freeze or crash. Requires a hard reboot.",
        alternative: None,
    },
    SafetyRule {
        pattern: "rm -rf /",
        level: DangerLevel::Critical,
        label: "Root Deletion",
        explanation: "Recursively deletes everything from the filesystem root.",
        consequence: "Complete system destruction. Unrecoverable without a reinstall.",
        alternative: None,
    },
    SafetyRule {
        pattern: "rm -rf /*",
        level: DangerLevel::Critical,
        label: "Root Glob Deletion",
        explanation: "Deletes all files and directories directly under /.",
        consequence: "Destroys the operating system. Unrecoverable.",
        alternative: None,
    },
    SafetyRule {
        pattern: "mkfs",
        level: DangerLevel::Critical,
        label: "Format Filesystem",
        explanation: "Creates (formats) a new filesystem on a device.",
        consequence: "All data on the target device is permanently destroyed.",
        alternative: Some("Use a partition manager like gparted to format safely"),
    },
    SafetyRule {
        pattern: "wipefs",
        level: DangerLevel::Critical,
        label: "Wipe Filesystem Signatures",
        explanation: "Erases filesystem signatures from a block device.",
        consequence: "The device becomes unreadable until reformatted. Data loss.",
        alternative: None,
    },
    SafetyRule {
        pattern: "> /dev/sd",
        level: DangerLevel::Critical,
        label: "Raw Device Write",
        explanation: "Writes raw data directly to a block device, bypassing the filesystem.",
        consequence: "Overwrites disk sectors; permanent data loss.",
        alternative: None,
    },
    SafetyRule {
        pattern: "sudo dd",
        level: DangerLevel::Critical,
        label: "Privileged Raw Copy",
        explanation: "Runs dd (raw disk copy tool) as root.",
        consequence: "Can overwrite entire disks if the target (of=) is wrong.",
        alternative: Some("Verify the `of=` target carefully before running"),
    },
    SafetyRule {
        pattern: "rm -rf",
        level: DangerLevel::High,
        label: "Recursive Force Delete",
        explanation: "Recursively deletes a directory tree without any prompts.",
        consequence: "Permanent deletion. No recycle bin. Use with extreme care.",
        alternative: Some("trash-cli: `trash <path>` sends to trash instead"),
    },
    SafetyRule {
        pattern: "rm -fr",
        level: DangerLevel::High,
        label: "Recursive Force Delete",
        explanation: "Equivalent to `rm -rf`. Recursively force-deletes without prompts.",
        consequence: "Permanent deletion. No recycle bin.",
        alternative: Some("trash-cli: `trash <path>`"),
    },
    SafetyRule {
        pattern: "dd ",
        level: DangerLevel::High,
        label: "Raw Disk Copy",
        explanation: "Copies raw bytes from one file/device to another.",
        consequence: "Writing to the wrong `of=` target will destroy data.",
        alternative: Some("Double-check `if=` and `of=` before running"),
    },
    SafetyRule {
        pattern: "iptables -F",
        level: DangerLevel::High,
        label: "Flush Firewall",
        explanation: "Removes all iptables rules, leaving the system with no firewall.",
        consequence: "All ports become reachable. Security risk on public networks.",
        alternative: Some("Use `iptables -L` to review before flushing"),
    },
    SafetyRule {
        pattern: "nft flush ruleset",
        level: DangerLevel::High,
        label: "Flush nftables",
        explanation: "Removes all nftables rules.",
        consequence: "All ports become reachable. Security risk on public networks.",
        alternative: Some("Use `nft list ruleset` to review first"),
    },
    SafetyRule {
        pattern: "sudo rm",
        level: DangerLevel::High,
        label: "Privileged Deletion",
        explanation: "Runs rm as root, bypassing normal user permission checks.",
        consequence: "Can delete system files, causing OS instability.",
        alternative: None,
    },
    SafetyRule {
        pattern: "shred",
        level: DangerLevel::High,
        label: "Secure Erasure",
        explanation: "Overwrites file data multiple times to prevent recovery.",
        consequence: "Files are gone permanently — even forensic recovery may fail.",
        alternative: None,
    },
    SafetyRule {
        pattern: "chmod -R 777",
        level: DangerLevel::High,
        label: "World-Writable Recursive",
        explanation: "Makes a directory tree writable and executable by all users.",
        consequence: "Any user can modify or execute your files. Security vulnerability.",
        alternative: Some("Use the minimum permissions needed, e.g. chmod 755"),
    },
    SafetyRule {
        pattern: "truncate --size=0",
        level: DangerLevel::High,
        label: "Truncate to Zero",
        explanation: "Sets a file's size to 0 bytes, wiping all content.",
        consequence: "All file content is permanently lost.",
        alternative: Some("> /path/to/backup first, then truncate"),
    },
    SafetyRule {
        pattern: "chmod 777",
        level: DangerLevel::Medium,
        label: "World-Writable",
        explanation: "Makes a file readable, writable, and executable by every user.",
        consequence: "Any user on the system can modify or run this file.",
        alternative: Some("Use 755 for executables, 644 for data files"),
    },
    SafetyRule {
        pattern: "sudo chmod",
        level: DangerLevel::Medium,
        label: "Privileged Permission Change",
        explanation: "Changes file permissions as root.",
        consequence: "Incorrect permissions on system files can break the OS.",
        alternative: None,
    },
    SafetyRule {
        pattern: "sudo chown",
        level: DangerLevel::Medium,
        label: "Privileged Ownership Change",
        explanation: "Changes file ownership as root.",
        consequence: "Changing ownership of system files can break services.",
        alternative: None,
    },
    SafetyRule {
        pattern: "curl | bash",
        level: DangerLevel::Medium,
        label: "Remote Code Execution",
        explanation: "Downloads and immediately executes a script from the internet.",
        consequence: "The remote script can do anything. Risk of supply-chain attack.",
        alternative: Some("Download first, inspect, then run: curl -o script.sh && bash script.sh"),
    },
    SafetyRule {
        pattern: "wget -O- | bash",
        level: DangerLevel::Medium,
        label: "Remote Code Execution",
        explanation: "Downloads and immediately executes a script from the internet.",
        consequence: "The remote script can do anything. Risk of supply-chain attack.",
        alternative: Some("Download first, inspect, then run"),
    },
];

// ── Public API ────────────────────────────────────────────────────────────────

/// A matched safety concern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConcern {
    /// The matched rule's danger level.
    pub level: DangerLevel,
    /// Short label (e.g. "Fork Bomb", "Recursive Force Delete").
    pub label: String,
    /// Human-readable explanation of the operation.
    pub explanation: String,
    /// What can go wrong.
    pub consequence: String,
    /// Optional safer alternative.
    pub alternative: Option<String>,
}

/// The result of analysing a command string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyReport {
    /// The command that was analysed.
    pub command: String,
    /// Every concern that was matched.
    pub concerns: Vec<SafetyConcern>,
    /// The highest danger level found (`None` if no concerns).
    pub max_level: Option<DangerLevel>,
    /// Whether this command should be blocked by the configured threshold.
    pub should_block: bool,
}

impl SafetyReport {
    /// Returns `true` if no safety concerns were found.
    pub fn is_clean(&self) -> bool {
        self.concerns.is_empty()
    }
}

// ── SafetyChecker ─────────────────────────────────────────────────────────────

/// Analyses shell commands against the built-in rule table.
///
/// Construct once and reuse across many commands — the rules are
/// loaded from the static table and require no heap allocation per analysis.
#[derive(Debug, Clone)]
pub struct SafetyChecker {
    /// Minimum danger level that sets `should_block = true` in the report.
    pub block_threshold: DangerLevel,
}

impl Default for SafetyChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl SafetyChecker {
    /// Create a checker that blocks `High` and `Critical` commands by default.
    pub fn new() -> Self {
        Self {
            block_threshold: DangerLevel::High,
        }
    }

    /// Create a checker with a custom block threshold.
    pub fn with_threshold(level: DangerLevel) -> Self {
        Self { block_threshold: level }
    }

    /// Analyse `command` and return a [`SafetyReport`].
    ///
    /// All matching rules are returned; duplicates (same pattern in the same
    /// command) are deduplicated by label.
    /// Analyse `command` and return a [`SafetyReport`].
    ///
    /// All matching rules are returned; duplicates (same pattern in the same
    /// command) are deduplicated by label.
    pub fn analyse(&self, command: &str) -> SafetyReport {
        let mut concerns = Vec::new();
        let mut seen_labels = std::collections::HashSet::new();

        for rule in RULES {
            if matches_rule(command, rule) && seen_labels.insert(rule.label) {
                concerns.push(SafetyConcern {
                    level: rule.level,
                    label: rule.label.to_string(),
                    explanation: rule.explanation.to_string(),
                    consequence: rule.consequence.to_string(),
                    alternative: rule.alternative.map(|s| s.to_string()),
                });
            }
        }

        let max_level = concerns.iter().map(|c| c.level).max();
        let should_block = max_level
            .map(|l| l >= self.block_threshold)
            .unwrap_or(false);

        SafetyReport {
            command: command.to_string(),
            concerns,
            max_level,
            should_block,
        }
    }
}

/// Helper function to check if a command matches a safety rule.
fn matches_rule(command: &str, rule: &SafetyRule) -> bool {
    let normalized_cmd = normalize_whitespace(command);
    let normalized_pattern = normalize_whitespace(rule.pattern);

    if command.contains(rule.pattern) || normalized_cmd.contains(&normalized_pattern) {
        return true;
    }

    if rule.label == "Remote Code Execution" || rule.pattern.contains('|') {
        if is_remote_code_execution(command) {
            return true;
        }
    }

    false
}

fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_remote_code_execution(command: &str) -> bool {
    let lower = command.to_lowercase();

    // Check A: Substitution & Eval RCE
    if is_substitution_rce(&lower) {
        return true;
    }

    // Check B: Piped RCE (Fetcher piped to Interpreter)
    let segments: Vec<&str> = lower.split('|').map(|s| s.trim()).collect();
    if segments.len() > 1 {
        let mut fetcher_seen = false;

        for seg in segments {
            let cmd_name = extract_effective_command(seg);

            if is_fetcher(&cmd_name) {
                fetcher_seen = true;
            } else if fetcher_seen && is_shell_or_interpreter(&cmd_name) {
                return true;
            }
        }
    }

    false
}

fn is_fetcher(cmd: &str) -> bool {
    matches!(cmd, "curl" | "wget" | "fetch")
}

fn is_shell_or_interpreter(cmd: &str) -> bool {
    matches!(
        cmd,
        "bash" | "sh" | "zsh" | "dash" | "fish" | "csh" | "tcsh" | "python" | "python2" | "python3" | "perl" | "ruby" | "php" | "node"
    )
}

fn extract_effective_command(segment: &str) -> String {
    let tokens: Vec<&str> = segment.split_whitespace().collect();
    if tokens.is_empty() {
        return String::new();
    }

    let mut idx = 0;
    while idx < tokens.len() {
        let token = tokens[idx];
        let base_name = token.rsplit('/').next().unwrap_or(token);

        if base_name == "sudo" {
            idx += 1;
            while idx < tokens.len() && tokens[idx].starts_with('-') {
                if tokens[idx] == "-u" || tokens[idx] == "-g" || tokens[idx] == "-C" {
                    idx += 2;
                } else {
                    idx += 1;
                }
            }
        } else if base_name == "env" {
            idx += 1;
            while idx < tokens.len() && tokens[idx].starts_with('-') {
                idx += 1;
            }
        } else {
            break;
        }
    }

    if idx < tokens.len() {
        let target = tokens[idx];
        target.rsplit('/').next().unwrap_or(target).to_string()
    } else {
        String::new()
    }
}

fn is_substitution_rce(lower: &str) -> bool {
    if let Some(pos) = lower.find("<(") {
        let rest = &lower[pos + 2..];
        let inner_cmd = extract_effective_command(rest);
        if is_fetcher(&inner_cmd) {
            return true;
        }
    }

    if let Some(pos) = lower.find("$(") {
        let rest = &lower[pos + 2..];
        let inner_cmd = extract_effective_command(rest);
        if is_fetcher(&inner_cmd) {
            return true;
        }
    }

    if let Some(pos) = lower.find('`') {
        let rest = &lower[pos + 1..];
        let inner_cmd = extract_effective_command(rest);
        if is_fetcher(&inner_cmd) {
            return true;
        }
    }

    if lower.contains("eval") {
        let tokens: Vec<&str> = lower.split_whitespace().collect();
        if !tokens.is_empty() {
            let base_cmd = tokens[0].rsplit('/').next().unwrap_or(tokens[0]);
            if base_cmd == "eval" && (lower.contains("curl") || lower.contains("wget") || lower.contains("fetch")) {
                return true;
            }
        }
    }

    false
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_command_produces_no_concerns() {
        let checker = SafetyChecker::new();
        let report = checker.analyse("ls -la /home");
        assert!(report.is_clean());
        assert!(!report.should_block);
        assert_eq!(report.max_level, None);
    }

    #[test]
    fn rm_rf_detected_as_high() {
        let checker = SafetyChecker::new();
        let report = checker.analyse("rm -rf ./build");
        assert!(!report.is_clean());
        let levels: Vec<DangerLevel> = report.concerns.iter().map(|c| c.level).collect();
        assert!(levels.contains(&DangerLevel::High));
    }

    #[test]
    fn fork_bomb_detected_as_critical() {
        let checker = SafetyChecker::new();
        let report = checker.analyse(":(){:|:&};:");
        assert!(!report.is_clean());
        assert_eq!(report.max_level, Some(DangerLevel::Critical));
        assert!(report.should_block);
    }

    #[test]
    fn curl_pipe_bash_is_medium() {
        let checker = SafetyChecker::new();
        let report = checker.analyse("curl https://example.com/install.sh | bash");
        assert!(!report.is_clean());
        let found_medium = report.concerns.iter().any(|c| c.level == DangerLevel::Medium);
        assert!(found_medium);
    }

    #[test]
    fn wget_pipe_sh_is_medium() {
        let checker = SafetyChecker::new();
        let report = checker.analyse("wget -O- https://example.com/setup.sh | sh");
        assert!(!report.is_clean());
        let found_medium = report.concerns.iter().any(|c| c.level == DangerLevel::Medium);
        assert!(found_medium);
    }

    #[test]
    fn block_threshold_respected() {
        let checker = SafetyChecker::with_threshold(DangerLevel::Critical);
        // rm -rf is High — should NOT block with a Critical threshold.
        let report = checker.analyse("rm -rf ./tmp");
        assert!(!report.is_clean());
        assert!(!report.should_block, "High should not block with Critical threshold");
    }

    #[test]
    fn deduplication_works() {
        let checker = SafetyChecker::new();
        // Command contains two patterns that map to the same label.
        let report = checker.analyse("rm -rf /foo && rm -rf /bar");
        let fork_count = report
            .concerns
            .iter()
            .filter(|c| c.label == "Recursive Force Delete")
            .count();
        assert_eq!(fork_count, 1, "Duplicate labels should be deduplicated");
    }

    #[test]
    fn test_rce_edge_cases() {
        let checker = SafetyChecker::new();

        // Must be detected as dangerous
        assert!(!checker.analyse("curl https://example.com/install.sh |bash").is_clean());
        assert!(!checker.analyse("curl https://example.com/install.sh |  bash").is_clean());
        assert!(!checker.analyse("curl https://example.com/install.sh |\tbash").is_clean());
        assert!(!checker.analyse("curl https://example.com/install.sh | /bin/bash").is_clean());
        assert!(!checker.analyse("curl https://example.com/install.sh | /bin/sh").is_clean());
        assert!(!checker.analyse("curl https://example.com/install.sh | sudo /bin/bash").is_clean());
        assert!(!checker.analyse("curl https://example.com/install.sh | sudo -u root bash").is_clean());
        assert!(!checker.analyse("curl https://example.com/install.sh | env bash").is_clean());
        assert!(!checker.analyse("curl https://example.com/install.sh | python3").is_clean());
        assert!(!checker.analyse("bash <(curl -sSL https://example.com)").is_clean());
        assert!(!checker.analyse("sh -c \"$(curl -fsSL https://example.com)\"").is_clean());
        assert!(!checker.analyse("eval \"$(curl -fsSL https://example.com)\"").is_clean());

        // Must NOT be detected (benign commands)
        assert!(checker.analyse("echo \"curl is awesome\" | bash").is_clean());
        assert!(checker.analyse("curl --help").is_clean());
        assert!(checker.analyse("curl https://example.com -o script.sh").is_clean());
    }
}
