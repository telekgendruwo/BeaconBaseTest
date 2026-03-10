# Security Policy

## Supported Versions

The current stable version of Beacon is actively supported with security updates.

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a vulnerability in Beacon, please report it privately to [me@davidnzube.xyz] instead of opening a public issue. We will work to address it promptly.

## Known Issues and Warnings

While actively developed, Beacon currently has the following known security-related warnings and a vulnerability, primarily stemming from transitive dependencies:

### 1. `ring` Crate Vulnerability (RUSTSEC-2025-0009)

*   **Crate:** `ring`
*   **Version:** `0.16.20`
*   **Title:** Some AES functions may panic when overflow checking is enabled.
*   **ID:** `RUSTSEC-2025-0009`
*   **Description:** This vulnerability in older versions of the `ring` cryptographic library can cause a program to panic (crash) under specific conditions related to AES operations when Rust's overflow checking is enabled. It is not a remote code execution (RCE) or data leakage vulnerability, but a potential denial-of-service vector.
*   **Impact:** A Beacon server instance could crash if triggered by a specially crafted input that causes the `ring` crate's AES functions to panic. This would lead to temporary unavailability of the service.
*   **Resolution Status:** We attempted to upgrade `ring` via direct dependency update and Cargo patching, but it is a complex transitive dependency, primarily through `ethers` crates via `jsonwebtoken`. Resolving this requires a major update of the `ethers` ecosystem dependencies or a custom patch, which is currently being investigated. The impact is assessed as moderate due to potential for panic/DoS rather than data compromise.

### 2. Unmaintained Crate Warnings

`cargo audit` identified several unmaintained crates within the dependency tree. While not immediate vulnerabilities, unmaintained crates carry a higher risk of not receiving security fixes for future discoveries.

*   **`fxhash` (RUSTSEC-2025-0057):** A hashing algorithm crate.
*   **`instant` (RUSTSEC-2024-0384):** A time-related utility crate.
*   **`ring` (RUSTSEC-2025-0010):** An older version of the `ring` crate (distinct from the vulnerability above, this warning is about general maintenance status).
*   **`rustls-pemfile` (RUSTSEC-2025-0134):** A utility for parsing PEM-encoded files used by `rustls`.

**Resolution Status:** We will monitor these dependencies for any newly reported vulnerabilities. Migration to maintained alternatives will be pursued as feasible with future dependency updates.

---
[your_contact_email@example.com]: mailto:me@davidnzube.xyz
