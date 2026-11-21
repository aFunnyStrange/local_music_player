# Cross-device music proposal

Status: requirements and alternatives only, not implemented. The desktop application remains Windows-only.

The goal is to reuse a PC library on a phone, potentially through a shared account, without repeatedly organizing the same music. An account or index does not make an offline PC file accessible: the phone needs audio from local storage, an online PC, a NAS or cloud storage.

Audio and small library records (names, favorites, playlists and identity) should be handled separately. Whole-library ZIP uploads are not required; incremental transfers, streaming and selected offline caches are alternatives.

| Option | Trade-off |
| --- | --- |
| Copy selected tracks to phone | Offline playback, but duplicate storage and incremental updates |
| Serve files from the PC | No central migration, but PC and network must remain available |
| NAS / home server | Always-on central storage with hardware and maintenance cost |
| Cloud / WebDAV / object storage | Independent of PC availability; evaluate cost, bandwidth and clients |

Evaluate existing software such as [Navidrome](https://www.navidrome.org/docs/overview/) and compatible mobile clients before creating a mobile app. Private remote networking such as Tailscale addresses reachability, not storage or PC availability. A web/PWA client is another candidate, requiring real-device checks for background audio, lock-screen controls, formats and offline caching.

Current track identity uses absolute file paths. A future design needs stable song identities and per-device or storage-location mappings; hashes are one candidate, with duplicate versions and tag changes needing explicit rules. Current display repairs do not rewrite audio tags, so third-party services will not automatically inherit them.

Local custom playlists now share track references and metadata; this is not cross-device synchronization. Future membership synchronization needs stable playlist IDs, song IDs, deletion semantics and conflict resolution.

Still undecided: Android/iOS, existing or custom clients, local versus remote streaming, offline quota, PC availability, storage budget, one user or multiple accounts, authentication, authoritative records and conflict handling. Audio transfer should support interruption recovery; do not require uploading every file before use.

After choosing a design, verify offline and reconnect behavior, file moves, duplicates, membership/name/favorite consistency, interrupted transfers and clear source-unavailable messages. Continue async-first design with bounded blocking work and no multiprocessing task model.
