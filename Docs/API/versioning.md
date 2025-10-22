# API Versioning & Deprecation

- Package versioning: each breaking change increments the proto package (e.g., `orca.v1` → `orca.v2`).
- Semantic versioning for SDKs: Major (breaking), Minor (features), Patch (fixes).
- URL/Service naming: service names include version suffix where relevant; binary compatibility not guaranteed across major versions.
- Deprecation policy: announce deprecation with timeline in release notes; maintain N-1 major version for a grace period.
- Envelope `protocol_version`: payload-level version to evolve envelope without changing transport.
- Compatibility testing: cross-version integration tests ensure SDKs detect/handle version mismatches.
