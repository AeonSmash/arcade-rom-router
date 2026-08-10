# Arcade ROM Router
## Cursor-Ready Product, Architecture, and Implementation Specification

**Working name:** Arcade ROM Router  
**Document type:** Master implementation specification  
**Primary target:** Windows 10/11 desktop  
**Primary use case:** Scan a user-owned folder of legacy MAME/arcade ROM sets, identify which emulator/core definition each set matches, explain failures, and launch each playable game with the correct emulator automatically.  
**Status:** Build specification  
**Prepared:** 2026-08-09

---

# 0. Cursor Instructions

This document is the source of truth for the project.

Build the application described here incrementally. Do not reinterpret this as a generic ROM frontend. The differentiating feature is **evidence-based ROM identification and automatic emulator routing across mixed historical arcade ROM-set generations**.

## Non-negotiable principles

1. **Never modify the user's source ROM collection by default.**
2. **Never download copyrighted ROMs, BIOS files, CHDs, keys, or other protected game data.**
3. Treat the user's original ROM folder as **read-only evidence**.
4. Use **DAT/checksum matching** as the primary compatibility detector.
5. Do not determine compatibility by blindly launching every ROM against every emulator.
6. A failed launch is diagnostic evidence, not the primary identification system.
7. Prefer deterministic, explainable results over opaque heuristics.
8. Every automatic route must retain:
   - the selected emulator/core,
   - why it was selected,
   - the matching DAT/database version,
   - confidence,
   - dependencies,
   - any user override.
9. The user must be able to override any automatic route.
10. Any future “repair” or “rebuild” functionality must operate on a **separate destination folder**, never in place.
11. The project must remain usable completely offline after required emulator cores and DAT files are installed.
12. Do not bundle ROMs, copyrighted BIOS images, or CHDs in the repository, installer, tests, screenshots, fixtures, or releases.
13. Maintain a living `PROGRESS.md` file as implementation proceeds.
14. Add automated tests before expanding into optional metadata/artwork integrations.
15. Favor reliability and recoverability over cleverness.

---

# 1. Product Vision

The user may have a folder accumulated across many years containing arcade ZIP files from different MAME generations. Some sets may correspond to MAME 0.78-era data, some to MAME 0.139, some to newer MAME, some may be compatible with FinalBurn Neo, and some may be incomplete or depend on parent, BIOS, device, sample, or CHD data.

The application turns that mixed folder into one coherent arcade library.

The user should be able to point the application at:

```text
D:\Arcade\Original-ROMs\
```

and ultimately see:

```text
1,427 arcade archives scanned

Playable                         1,103
Playable with alternate route      94
Missing dependency                121
Wrong/incomplete ROM set           63
CHD required                       19
Unidentified                       27
```

The core promise is:

> **The user chooses a game. The application chooses the correct configured emulator route.**

The user should not need to remember whether a specific game belongs to MAME 2003-Plus, MAME 2010, FBNeo, MAME Current, or another installed route.

---

# 2. Problem Definition

Arcade emulation differs from typical cartridge-console emulation.

An arcade `.zip` is often a **ROM set** containing multiple chip dumps. Compatibility depends on the exact ROM definitions expected by a particular emulator build or core. Definitions may change over time as dumps are corrected, renamed, split, merged, or reclassified.

A mixed historical collection therefore creates several common failure conditions:

- archive belongs to an older ROM-set generation;
- archive is incomplete;
- parent ROM set is missing;
- BIOS set is missing;
- device ROM is missing;
- CHD is missing;
- samples are missing;
- ZIP filename no longer matches the expected set name;
- clone/parent relationship differs;
- ROM contents are valid but better match a different emulator/core;
- ROM is recognized by checksum but cannot be associated confidently with one complete playable set;
- a valid archive works in multiple cores;
- emulator core is installed but required system files are not;
- controller mapping makes a technically working game appear broken;
- core update changes the expected ROM definition.

The application must separate these issues and explain them accurately.

---

# 3. Product Goals

## 3.1 Primary goals

The application shall:

1. Scan one or more user-selected arcade ROM folders.
2. Inventory archives without extracting them permanently.
3. Read ZIP contents directly.
4. Record archive member:
   - filename,
   - uncompressed size,
   - CRC32,
   - optional SHA-1,
   - optional SHA-256 for local file identity.
5. Import ROM definitions from DAT/XML sources.
6. Match local archive contents against emulator-specific definitions.
7. Determine dependencies:
   - parent,
   - BIOS,
   - device,
   - CHD,
   - sample where applicable.
8. Classify every detected game into an explicit compatibility state.
9. Rank valid emulator/core routes.
10. Launch through RetroArch or optional standalone MAME without user core selection.
11. Integrate controller-oriented navigation.
12. Provide a polished desktop arcade library.
13. Explain why a non-working ROM does not work.
14. Cache scan results so large collections do not need full re-analysis on every launch.
15. Rescan incrementally when files change.

## 3.2 Secondary goals

- Favorites.
- Recently played.
- Play count.
- Per-game route override.
- Per-game controller notes/profile.
- Big-picture / couch mode.
- Local artwork folders.
- Optional metadata providers.
- Multiple ROM roots.
- Multiple emulator installations.
- Exportable diagnostic report.
- Exportable compatibility CSV/JSON.
- Audit log.
- CHD association.
- DAT management.
- Core health check.

## 3.3 Explicit non-goals for MVP

Do not make the MVP into:

- a ROM downloader;
- a BIOS downloader;
- a piracy search tool;
- a general console emulator frontend;
- a full ClrMamePro replacement;
- a ROM patching system;
- a ROM editor;
- an archive repair tool;
- a cloud account service;
- a multiplayer platform;
- a shader manager;
- a save-state synchronizer;
- a RetroAchievements client;
- a scraper-dependent frontend.

Those can be evaluated later if appropriate.

---

# 4. Technical Basis

The application should be built around emulator-provided or community-maintained **machine-readable ROM definitions**, generally DAT/XML-style metadata.

These definitions can describe:

- machine/set name;
- description/title;
- year;
- manufacturer;
- clone-of relationship;
- ROM-of relationship;
- required ROM member names;
- member sizes;
- CRCs;
- SHA-1 hashes;
- BIOS relationships;
- device relationships;
- disk/CHD data;
- status flags.

For modern standalone MAME, use supported command-line metadata and verification capabilities where useful.

Useful MAME concepts include:

```text
mame -verifyroms
mame -romident
mame -listxml
```

The application must not assume command output is forever identical. Wrap command invocation behind adapters and cover parsers with fixtures.

RetroArch supports launching content with an explicitly selected core:

```text
retroarch.exe -L "C:\...\cores\<core>_libretro.dll" "D:\Arcade\rom.zip"
```

This direct invocation is important because the Router, not a single global playlist association, decides the emulator route per game.

---

# 5. Recommended Technology Stack

## 5.1 Desktop shell

**Tauri 2 + Rust backend + React + TypeScript frontend**

Reasons:

- native filesystem access;
- efficient process spawning;
- small desktop package compared with an Electron-first approach;
- Rust is well-suited to ZIP enumeration, hashing, XML parsing, SQLite access, process supervision, and concurrent scanning;
- React/TypeScript is appropriate for a polished library interface;
- Windows-first while preserving future macOS/Linux portability.

Do not hard-pin package versions in this document. At implementation time use compatible current stable releases and commit lockfiles.

## 5.2 Frontend

- React
- TypeScript
- Vite
- CSS variables/design tokens
- accessible component primitives
- virtualized game grid/list
- state store such as Zustand if state complexity warrants it
- TanStack Query only if it materially improves local async state; do not add it gratuitously

## 5.3 Backend

Rust modules:

```text
scanner
archive
hashing
dat
matcher
dependency
routing
launch
retroarch
mame
controllers
library
diagnostics
settings
db
logging
jobs
```

## 5.4 Database

SQLite.

Use migrations from the beginning.

Recommended access:

- `sqlx` or another mature SQLite crate;
- WAL mode;
- foreign keys enabled;
- transactions for scan batches.

## 5.5 Logging

Use structured logs.

Log levels:

```text
ERROR
WARN
INFO
DEBUG
TRACE
```

Default production UI should expose INFO-level diagnostic history while keeping noisy data in rotating local log files.

---

# 6. Proposed Repository Structure

```text
arcade-rom-router/
├─ README.md
├─ SPEC.md
├─ PROGRESS.md
├─ CHANGELOG.md
├─ LICENSE
├─ package.json
├─ src/
│  ├─ app/
│  ├─ components/
│  ├─ features/
│  │  ├─ onboarding/
│  │  ├─ library/
│  │  ├─ game-detail/
│  │  ├─ scanner/
│  │  ├─ diagnostics/
│  │  ├─ emulator-manager/
│  │  ├─ dat-manager/
│  │  ├─ controller-center/
│  │  └─ settings/
│  ├─ hooks/
│  ├─ stores/
│  ├─ styles/
│  ├─ types/
│  └─ utils/
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ commands/
│  │  ├─ scanner/
│  │  ├─ archive/
│  │  ├─ dat/
│  │  ├─ matcher/
│  │  ├─ routing/
│  │  ├─ launch/
│  │  ├─ emulator/
│  │  ├─ controller/
│  │  ├─ diagnostics/
│  │  ├─ db/
│  │  └─ settings/
│  ├─ migrations/
│  ├─ Cargo.toml
│  └─ tauri.conf.json
├─ tests/
│  ├─ fixtures/
│  │  ├─ dat/
│  │  ├─ archives/
│  │  ├─ mame-output/
│  │  └─ retroarch-output/
│  └─ integration/
└─ docs/
   ├─ architecture/
   ├─ dat-format/
   ├─ routing/
   ├─ troubleshooting/
   └─ screenshots/
```

---

# 7. Emulator/Core Support Model

Do not hardcode a single universal MAME target.

Represent every emulator/core as a configurable **Emulator Profile**.

Example built-in profile templates:

| Profile | Typical ROM-definition family | Runner |
|---|---|---|
| FinalBurn Neo | FBNeo current matching DAT | RetroArch |
| MAME 2003-Plus | Based substantially on MAME 0.78 with Plus updates | RetroArch |
| MAME 2003 | MAME 0.78 | RetroArch |
| MAME 2010 | MAME 0.139 | RetroArch |
| MAME 2015 | MAME 0.160 | RetroArch |
| MAME 2016 | MAME 0.174 | RetroArch |
| MAME Current | current installed libretro MAME core definition | RetroArch |
| Standalone MAME | current installed MAME definition | native executable |

The profile database must be extensible.

Example conceptual profile:

```json
{
  "id": "mame2003plus",
  "displayName": "MAME 2003-Plus",
  "runnerType": "retroarch",
  "corePath": "C:\\RetroArch\\cores\\mame2003_plus_libretro.dll",
  "datSources": [
    {
      "type": "xml-dat",
      "path": "C:\\ArcadeRouter\\dats\\mame2003-plus.xml"
    }
  ],
  "priority": 60,
  "enabled": true,
  "capabilities": {
    "zip": true,
    "chd": true,
    "samples": true
  }
}
```

Do not assume filenames above without detecting actual installed files.

---

# 8. Emulator Discovery

## 8.1 RetroArch discovery

On Windows, support:

1. user-selected executable;
2. common install locations;
3. portable folder selection;
4. previously saved valid location.

Validation:

- executable exists;
- can be started with a harmless information/help command;
- core directory is readable;
- selected core DLL exists;
- configuration directory can be identified;
- system directory can be resolved when possible.

Never silently rewrite the user's RetroArch configuration.

## 8.2 Core discovery

Scan the configured RetroArch core directory for known arcade core identifiers.

Examples may include names containing:

```text
fbneo
mame2003
mame2003_plus
mame2010
mame2015
mame2016
mame
```

Do not infer exact compatibility from DLL filename alone. A core must also have an associated DAT/definition source before it can receive a high-confidence ROM route.

## 8.3 Standalone MAME discovery

Allow manual selection of `mame.exe`.

Run an adapter health check.

Potential capabilities:

```text
-listxml
-verifyroms
-romident
-help / -showusage
```

Record the discovered MAME version string.

Do not assume that the installed standalone MAME version matches a RetroArch MAME core.

---

# 9. DAT / ROM Definition Manager

The DAT Manager is central to the application.

## 9.1 Responsibilities

- import DAT/XML;
- parse metadata;
- fingerprint DAT;
- associate DAT with emulator profile;
- record source and version;
- detect duplicate imports;
- support replacement/upgrade;
- re-run only affected matches after DAT update.

## 9.2 DAT metadata

Store:

```text
id
emulator_profile_id
display_name
source_type
source_path
source_url_optional
declared_version
imported_at
file_sha256
machine_count
rom_entry_count
disk_entry_count
parser_version
active
```

## 9.3 DAT sources

MVP should support:

1. standard XML-style arcade DATs;
2. MAME `-listxml` output imported as an active definition snapshot.

Later:

- Logiqx DAT variants;
- FBNeo-specific metadata improvements;
- custom compatibility packs.

## 9.4 Versioning rule

Never overwrite historical DAT metadata silently.

If a core is updated and its DAT changes:

- import new DAT;
- keep previous scan evidence;
- invalidate compatibility results tied to the old active DAT;
- rescore affected ROMs;
- show a notification:
  - “Core definition changed; 238 games require compatibility re-evaluation.”

---

# 10. ROM Folder Model

A ROM root is a user-selected location.

Example:

```text
D:\Arcade\Original-ROMs\
```

Properties:

```text
id
path
label
recursive
read_only_mode
enabled
last_scan_at
watch_changes
```

Default:

```text
read_only_mode = true
```

The scanner may read files and metadata but must not:

- rename;
- delete;
- move;
- rewrite;
- recompress;
- extract permanently;
- patch.

---

# 11. Supported Content Types

## MVP

```text
.zip
.chd
```

CHDs normally belong in game-specific directories rather than being treated as independent games.

## Phase 2

```text
.7z
```

Add only after archive library behavior and performance are verified.

## Ignored by default

```text
.txt
.nfo
.jpg
.png
.ini
.cfg
.exe
.dll
.bat
.cmd
```

Never execute files discovered in a ROM directory.

---

# 12. Scan Pipeline

The scanner must be incremental and cancellable.

## 12.1 High-level flow

```text
Select ROM root
    ↓
Enumerate candidate files
    ↓
Compare file metadata against scan cache
    ↓
Inspect new/changed archives
    ↓
Read archive member metadata
    ↓
Compute local fingerprints
    ↓
Generate DAT candidates
    ↓
Match content against expected ROM definitions
    ↓
Resolve parent/BIOS/device/CHD dependencies
    ↓
Generate compatibility results
    ↓
Rank routes
    ↓
Persist
    ↓
Update UI incrementally
```

## 12.2 File identity

For each archive:

```text
absolute_path
canonical_path
file_name
extension
size_bytes
modified_time
quick_signature
sha256_optional
```

A quick signature may combine:

```text
path + size + modified_time
```

Use full SHA-256 when:

- duplicate detection is enabled;
- file changed unexpectedly;
- archive identity needs stronger proof;
- user runs “Deep Verify.”

Do not hash multi-gigabyte files unnecessarily during normal startup.

## 12.3 ZIP inspection

Read central directory without extracting to disk.

Record each member:

```text
archive_id
member_name
uncompressed_size
compressed_size
crc32
compression_method
is_directory
```

Use member CRC + size as primary ROM-chip match evidence.

Optionally compute SHA-1 for individual members only if needed by a DAT and not otherwise obtainable.

## 12.4 Error tolerance

Malformed archives must not crash a scan.

Classification example:

```text
ARCHIVE_UNREADABLE
```

Store exact parse error in diagnostics.

---

# 13. Compatibility Matching Engine

This is the most important backend subsystem.

## 13.1 Rule: identify, do not guess

A ROM archive should receive a route only if there is sufficient evidence.

Evidence sources, strongest first:

1. complete required member checksum match to a DAT machine;
2. complete checksum/size match despite archive naming difference;
3. current MAME verifier confirms set;
4. strong checksum subset plus resolvable parent/BIOS dependencies;
5. filename-only match;
6. fuzzy title/name guess.

Only levels 1–4 may produce automatic runnable routes.

Filename-only and fuzzy matches are for diagnostics, not automatic launching.

## 13.2 Matching keys

For each local member:

```text
crc32
size
member_name
```

Optional:

```text
sha1
```

Create indexed lookup tables so one CRC can efficiently identify candidate machine definitions.

## 13.3 Machine match computation

For each candidate machine:

```text
required_rom_count
matched_required_count
missing_required_count
wrong_size_count
wrong_crc_count
optional_rom_count
matched_optional_count
parent_required
bios_required
device_required
chd_required
```

## 13.4 Compatibility score

Do not use a single score without retaining the underlying facts.

Conceptual score:

```text
base = 0

+ 70 complete required ROM checksum match
+ 15 archive name matches set name
+  5 all required dependency archives present
+  5 required CHDs present and valid enough for configured validation level
+  5 emulator/core health check passes

- 50 any required ROM mismatch
- 40 required parent unresolved
- 40 required BIOS unresolved
- 40 required device ROM unresolved
- 40 required CHD unresolved
- 20 archive unreadable
```

However, compatibility state should be derived from explicit rules, not merely thresholds.

---

# 14. Compatibility States

Use strongly typed states.

```text
VERIFIED_PLAYABLE
VERIFIED_PLAYABLE_WITH_DEPENDENCIES
MULTIPLE_VALID_ROUTES
MISSING_PARENT
MISSING_BIOS
MISSING_DEVICE
MISSING_CHD
MISSING_SAMPLES_OPTIONAL
INCOMPLETE_SET
WRONG_ROM_REVISION
KNOWN_SET_NAME_UNVERIFIED_CONTENT
RECOGNIZED_ROM_CONTENT_AMBIGUOUS_SET
ARCHIVE_UNREADABLE
UNIDENTIFIED
EMULATOR_NOT_INSTALLED
CORE_NOT_INSTALLED
DAT_NOT_INSTALLED
ROUTE_UNAVAILABLE
USER_DISABLED
```

A game may have multiple diagnostic flags but one primary library state.

Example:

```json
{
  "primaryState": "MISSING_BIOS",
  "flags": [
    "CHECKSUMS_MATCH_GAME_SET",
    "BIOS_NEOGEO_REQUIRED",
    "BIOS_NEOGEO_NOT_FOUND"
  ]
}
```

---

# 15. Dependency Resolution

Arcade dependencies are first-class objects.

## 15.1 Parent/clone

DAT example concept:

```text
cloneof = parent_set
romof   = parent_set
```

When a split clone lacks data supplied by its parent:

```text
clone archive + parent archive = complete launch context
```

The UI should show:

```text
Street Fighter II [clone]
Game archive: present
Parent: missing
Required parent set: <setname>
```

Do not tell the user to download it from an unauthorized source.

## 15.2 BIOS

BIOS dependencies should show separately from parent dependencies.

Example status language:

```text
The game ROM contents match this core, but a required BIOS set is not present in the configured search path.
```

## 15.3 Device ROMs

Treat device dependencies distinctly when identified by the DAT/emulator.

## 15.4 CHD

Model:

```text
romroot\
  game.zip
  game\
    game.chd
```

Do not assume every core uses identical CHD path conventions; attach path rules to emulator profiles.

For MAME 2003-Plus, documentation indicates game-specific CHD subfolders are used.

## 15.5 Samples

Samples may affect audio without preventing launch.

Classify as:

```text
PLAYABLE_WITH_AUDIO_SAMPLE_WARNING
```

rather than “broken” unless the target core genuinely requires them to boot.

---

# 16. Historical ROM-Set Routing

The utility exists specifically to support a mixed legacy collection.

Example route candidates:

```text
ROM A
 ├─ FBNeo DAT: no match
 ├─ MAME 2003-Plus DAT: complete match
 ├─ MAME 2010 DAT: incomplete
 └─ MAME Current: no match

Selected:
MAME 2003-Plus
```

Another:

```text
ROM B
 ├─ FBNeo: complete match
 ├─ MAME Current: complete match
 └─ MAME 2003-Plus: complete match

Selected according to route preference policy.
```

The application must retain all valid candidates.

---

# 17. Route Selection Policy

Route selection must be deterministic and configurable.

## 17.1 Priority order

1. Per-game user override.
2. Explicit user global emulator preference.
3. Verified complete route with no missing dependency.
4. Active/maintained accuracy-oriented core preference.
5. Hardware/performance profile.
6. Stable tie-breaker.

## 17.2 Built-in preference modes

### Balanced / Recommended

Prefer:

1. FBNeo when the installed FBNeo DAT completely matches and the title is supported well;
2. current MAME when completely matched;
3. historical core with exact matching definition;
4. other exact verified route.

Do not route to a modern core merely because it is newer if the ROM definition does not match.

### Maximum Legacy Compatibility

Prefer the exact historical/core definition with strongest match and lowest dependency burden.

Useful for the user's existing old collection.

### Preservation / Accuracy

Prefer current MAME when the user's content matches it; otherwise explain why historical data is required.

### Performance

Prefer exact compatible lower-overhead core according to configurable profile.

## 17.3 Per-game overrides

User may select:

```text
Automatic
FBNeo
MAME Current
MAME 2003-Plus
MAME 2010
...
```

Override must show warning if chosen route is not verified.

Option:

```text
[ ] Allow launch even if unverified
```

Default false.

---

# 18. Confidence Model

Display a human-readable confidence label.

```text
Verified
Strong
Partial
Unknown
```

### Verified

All required ROM members match expected checksums/sizes for the selected definition and dependencies are satisfied.

### Strong

Game data matches but an external verifier or dependency state leaves minor uncertainty.

### Partial

Some ROM members identify strongly, but the set is incomplete or ambiguous.

### Unknown

Filename/title-only evidence or no reliable identification.

Never label filename-only matching as “Verified.”

---

# 19. Optional MAME Verification Adapter

For compatible standalone MAME profiles, use MAME commands as secondary verification/diagnostic tools.

## 19.1 `-verifyroms`

Use to validate sets visible in the configured MAME `rompath`.

Parse conservatively.

Store raw output for troubleshooting.

## 19.2 `-romident`

Useful for unknown or renamed archive content.

MAME documents return states in which all, some, or none of the ROMs are identified.

Do not treat “some files identified” as proof the archive is a complete runnable set.

## 19.3 `-listxml`

Use to generate/import a definition snapshot corresponding to the installed MAME executable.

Record:

```text
mame version
generated timestamp
xml sha256
machine count
```

This is preferable to assuming a modern external DAT is identical to the installed executable.

---

# 20. RetroArch Integration

RetroArch is the primary launch frontend.

## 20.1 Launch contract

Backend receives:

```json
{
  "gameId": 123,
  "routeId": 456
}
```

Backend resolves all paths internally.

Never accept arbitrary executable command strings directly from frontend state.

Construct an argument array, not a shell-concatenated command.

Example conceptual process spawn:

```text
executable:
C:\RetroArch\retroarch.exe

args:
-L
C:\RetroArch\cores\mame2003_plus_libretro.dll
D:\Arcade\Original-ROMs\game.zip
```

Use native process APIs.

Do not spawn through `cmd.exe`.

## 20.2 Configuration

Support:

```text
base RetroArch config
optional Router append config
per-core append config
optional per-game append config
```

Do not modify the user's main `retroarch.cfg` without explicit consent.

Prefer generated Router-owned config fragments stored under application data.

## 20.3 Verbose launch mode

Diagnostic launch:

```text
--verbose
--log-file <router-owned-log-path>
```

Exact supported CLI options should be validated against the installed RetroArch build.

Capture:

```text
process start time
process exit code
log path
selected core
content path
duration
```

## 20.4 Launch outcome

A process launch alone does not prove a game works.

Possible runtime status:

```text
LAUNCH_REQUESTED
PROCESS_STARTED
PROCESS_EXITED_NORMALLY
PROCESS_EXITED_ERROR
USER_CONFIRMED_WORKING
USER_REPORTED_PROBLEM
```

Add a post-play lightweight prompt only when useful:

```text
Did this game run correctly?
[Yes] [Had a problem] [Don't ask again]
```

User confirmation can improve route preference but must not replace checksum evidence.

---

# 21. Controller Support

The application should feel usable from a desk or a living-room arcade setup.

## 21.1 Division of responsibility

**Arcade ROM Router**
- controller detection in its UI;
- controller-friendly navigation;
- launch;
- per-game control profile metadata;
- diagnostics;
- optional shortcuts.

**RetroArch**
- actual emulation input;
- controller driver;
- RetroPad abstraction;
- device-specific autoconfiguration;
- per-core/per-game remaps.

Do not unnecessarily duplicate RetroArch's mature input system.

## 21.2 Controller center

Screen:

```text
CONTROLLERS

Xbox Wireless Controller
Status: Connected
RetroArch profile: Detected
Port: 1
Test: [Open]

8BitDo Arcade Stick
Status: Connected
RetroArch profile: Detected
Port: 2
Test: [Open]
```

## 21.3 Browser/WebView controller detection

Use Gamepad API for Router UI where supported.

Fallback to keyboard/mouse if unavailable.

Backend does not need to own gameplay controller APIs.

## 21.4 Big-picture navigation

Default bindings:

```text
D-pad / Left stick   Navigate
A / South            Select / Launch
B / East             Back
X / West             Favorite
Y / North            Details
LB/RB                 Previous/next filter
Start                 Context menu
Select/View           Search
```

Bindings must be configurable.

## 21.5 Controller test

Visual panel:

```text
sticks
d-pad
face buttons
shoulders
triggers
start/select
```

Show live states.

This tests Router input, not necessarily RetroArch remapping.

## 21.6 Arcade control classifications

Store optional game control metadata:

```text
JOYSTICK_2WAY
JOYSTICK_4WAY
JOYSTICK_8WAY
DUAL_JOYSTICK
BUTTONS_1
BUTTONS_2
BUTTONS_3
BUTTONS_4
BUTTONS_6
TRACKBALL
SPINNER
DIAL
LIGHTGUN
ANALOG_STICK
PEDAL
OTHER
```

MAME 2003-Plus specifically supports 4-way simulation and analog/digital controls; expose relevant core capabilities only when supported.

## 21.7 Per-game remapping

MVP:

- open RetroArch Quick Menu instructions;
- record whether a custom remap exists;
- launch normally.

Phase 2:

- manage Router-owned remap templates only after exact RetroArch remap format behavior is validated;
- never overwrite an existing user remap silently.

---

# 22. Library Interface

The app should look like a premium arcade management application rather than a developer utility.

## 22.1 Design direction

Visual concept:

**“Modern arcade archive.”**

Characteristics:

- deep dark neutral background;
- restrained neon accents;
- luminous status chips;
- high-contrast text;
- game artwork as the visual focus;
- subtle CRT/scanline motifs only as decorative accents;
- no excessive fake scanlines over text;
- no illegible retro pixel font for body text;
- smooth 60 fps navigation;
- controller-first focus rings;
- large readable typography.

## 22.2 Main layout

```text
┌────────────────────────────────────────────────────────────┐
│ ARCADE ROM ROUTER     Search…         Controller ●     ⚙  │
├───────────────┬────────────────────────────────────────────┤
│ Library       │                                            │
│ Favorites     │     GAME GRID / LIST                       │
│ Recently      │                                            │
│ Working       │                                            │
│ Problems      │                                            │
│ Unidentified  │                                            │
│               │                                            │
│ Emulators     │                                            │
│ DATs          │                                            │
│ Controllers   │                                            │
│ Diagnostics   │                                            │
└───────────────┴────────────────────────────────────────────┘
```

---

# 23. Game Cards

Card should show:

```text
[artwork]

Street Fighter II
1991 • Capcom

✓ Verified
FBNeo
```

Problem card:

```text
Mortal Kombat
1992 • Midway

⚠ Missing dependency
MAME 2003-Plus
```

Unknown:

```text
rom1234.zip

? Unidentified
```

Do not fake game titles for unknown archives.

---

# 24. Library Filters

Filters:

```text
All
Playable
Problems
Unidentified
Favorites
Recently played
Never played
```

By emulator:

```text
FBNeo
MAME Current
MAME 2003-Plus
MAME 2010
...
```

By state:

```text
Verified
Missing parent
Missing BIOS
Missing CHD
Incomplete
Wrong revision
Unreadable
```

Metadata filters where known:

```text
year
manufacturer
genre/category
players
orientation
control type
```

---

# 25. Search

Search:

- display title;
- set name;
- parent name;
- manufacturer;
- filename;
- emulator route.

Support typo-tolerant title search in UI, but never use fuzzy search to establish compatibility.

---

# 26. Game Detail Screen

Example:

```text
STREET FIGHTER II

[large artwork]        Status: VERIFIED
                       Selected route: FBNeo
                       Confidence: Verified

Set name: sf2
Parent: —
Year: 1991
Manufacturer: Capcom

ROM VERIFICATION
12 / 12 required ROMs match
0 missing
0 mismatched

DEPENDENCIES
BIOS: Not required
Parent: Not required
CHD: Not required

VALID ROUTES
✓ FBNeo                  Verified
✓ MAME Current           Verified
△ MAME 2003-Plus         Partial

[PLAY] [Route ▼] [Diagnostics] [Favorite]
```

---

# 27. Diagnostics Screen

The diagnostics system is a major feature.

Bad:

```text
ROM doesn't work.
```

Good:

```text
The archive is recognized as <game> for MAME 2010.

8 required ROM chips were found.
2 required ROM chips are missing.

Missing entries:
  abc123.bin  CRC 12345678
  abc124.bin  CRC 87654321

The archive therefore cannot be verified as a complete MAME 2010 set.

A MAME 2003-Plus definition also recognizes 7 of the same chips,
but it does not form a complete valid set there either.
```

For parent:

```text
This clone set is valid, but it relies on parent set:
<parentname>

Parent set status:
Not found in any enabled ROM root.
```

For BIOS:

```text
Game ROM data matches.
Required BIOS:
<bios set name>

BIOS status:
Not found in the configured core search locations.
```

For CHD:

```text
ROM archive matches.
Required disk:
<disk name>

Expected location for this route:
D:\Arcade\Original-ROMs\<setname>\<diskname>.chd

Status:
Not found
```

---

# 28. Diagnostic Explainability

Every conclusion should have machine-readable reasons.

Example:

```json
{
  "gameId": 42,
  "route": "mame2010",
  "state": "MISSING_PARENT",
  "confidence": "VERIFIED_SET_CONTENT",
  "evidence": [
    {
      "type": "archive-name-match",
      "value": "gameclone"
    },
    {
      "type": "rom-checksum-match",
      "matched": 7,
      "expectedLocal": 7
    },
    {
      "type": "parent-required",
      "set": "gameparent"
    },
    {
      "type": "parent-not-found"
    }
  ]
}
```

The UI renders these facts into readable language.

---

# 29. Compatibility Database Schema

Suggested schema.

## `rom_roots`

```sql
id INTEGER PRIMARY KEY
path TEXT UNIQUE NOT NULL
label TEXT
recursive INTEGER NOT NULL
enabled INTEGER NOT NULL
read_only INTEGER NOT NULL
last_scan_at TEXT
```

## `archives`

```sql
id INTEGER PRIMARY KEY
rom_root_id INTEGER NOT NULL
path TEXT UNIQUE NOT NULL
file_name TEXT NOT NULL
extension TEXT NOT NULL
size_bytes INTEGER NOT NULL
modified_at TEXT
quick_signature TEXT
sha256 TEXT
archive_state TEXT NOT NULL
last_scanned_at TEXT NOT NULL
```

## `archive_members`

```sql
id INTEGER PRIMARY KEY
archive_id INTEGER NOT NULL
member_name TEXT NOT NULL
size_bytes INTEGER
crc32 TEXT
sha1 TEXT
```

Indexes:

```text
crc32
crc32 + size_bytes
archive_id
```

## `emulator_profiles`

```sql
id TEXT PRIMARY KEY
display_name TEXT NOT NULL
runner_type TEXT NOT NULL
executable_path TEXT
core_path TEXT
enabled INTEGER NOT NULL
priority INTEGER NOT NULL
settings_json TEXT
last_health_check TEXT
health_state TEXT
```

## `dat_sources`

```sql
id INTEGER PRIMARY KEY
emulator_profile_id TEXT NOT NULL
display_name TEXT NOT NULL
version TEXT
path TEXT NOT NULL
sha256 TEXT NOT NULL
imported_at TEXT NOT NULL
active INTEGER NOT NULL
parser_version INTEGER NOT NULL
```

## `machines`

```sql
id INTEGER PRIMARY KEY
dat_source_id INTEGER NOT NULL
set_name TEXT NOT NULL
description TEXT
year TEXT
manufacturer TEXT
clone_of TEXT
rom_of TEXT
is_bios INTEGER NOT NULL DEFAULT 0
runnable INTEGER
metadata_json TEXT
```

## `machine_roms`

```sql
id INTEGER PRIMARY KEY
machine_id INTEGER NOT NULL
name TEXT NOT NULL
size_bytes INTEGER
crc32 TEXT
sha1 TEXT
status TEXT
optional INTEGER NOT NULL DEFAULT 0
merge_name TEXT
bios_name TEXT
region TEXT
```

Indexes:

```text
machine_id
crc32
crc32 + size_bytes
sha1
```

## `machine_disks`

```sql
id INTEGER PRIMARY KEY
machine_id INTEGER NOT NULL
name TEXT NOT NULL
sha1 TEXT
status TEXT
optional INTEGER NOT NULL DEFAULT 0
```

## `match_results`

```sql
id INTEGER PRIMARY KEY
archive_id INTEGER NOT NULL
machine_id INTEGER NOT NULL
emulator_profile_id TEXT NOT NULL
dat_source_id INTEGER NOT NULL
state TEXT NOT NULL
confidence TEXT NOT NULL
matched_required INTEGER NOT NULL
missing_required INTEGER NOT NULL
wrong_required INTEGER NOT NULL
score REAL
evidence_json TEXT NOT NULL
created_at TEXT NOT NULL
```

## `routes`

```sql
id INTEGER PRIMARY KEY
archive_id INTEGER NOT NULL
machine_id INTEGER NOT NULL
emulator_profile_id TEXT NOT NULL
match_result_id INTEGER NOT NULL
is_selected INTEGER NOT NULL
selection_reason TEXT NOT NULL
user_override INTEGER NOT NULL DEFAULT 0
launchable INTEGER NOT NULL
```

## `play_history`

```sql
id INTEGER PRIMARY KEY
archive_id INTEGER NOT NULL
route_id INTEGER NOT NULL
started_at TEXT NOT NULL
ended_at TEXT
exit_code INTEGER
user_result TEXT
log_path TEXT
```

## `favorites`

```sql
archive_id INTEGER PRIMARY KEY
created_at TEXT NOT NULL
```

## `settings`

```sql
key TEXT PRIMARY KEY
value_json TEXT NOT NULL
```

---

# 30. Matching Performance

A large collection may contain thousands of archives and millions of DAT ROM entries.

Avoid O(archives × machines × members).

## 30.1 Candidate generation

For each archive member CRC:

```text
SELECT candidate machine IDs WHERE machine_rom.crc32 = local_crc
```

Count hits by machine.

Only deeply evaluate top candidate machines or those with set-name match.

## 30.2 Cache

If:

```text
path
size
modified time
DAT fingerprint
```

are unchanged, reuse match result.

## 30.3 Parallelism

Archive inspection can be parallelized.

Use a bounded worker pool.

Do not saturate all CPU cores by default.

Suggested setting:

```text
Automatic = min(logical_cores - 1, 8)
```

Minimum 1.

Make configurable.

## 30.4 UI progress

Report:

```text
Enumerating files
Inspecting archives
Matching ROM definitions
Resolving dependencies
Selecting routes
Finalizing library
```

Show:

```text
734 / 1,427
51%
```

Cancellation must preserve already committed valid scan data where safe.

---

# 31. Incremental Rescanning

On startup:

1. enumerate root;
2. compare path/size/mtime to DB;
3. scan only:
   - new;
   - changed;
   - missing/removed;
   - invalidated by DAT change.

Optional filesystem watcher:

- debounce changes;
- never rescan continuously while files are actively being copied;
- schedule a quiet-period scan.

---

# 32. Original Collection Protection

This deserves explicit implementation rules.

## 32.1 Source folder operations

Allowed:

```text
read directory
read archive metadata
read archive contents
hash
launch using source path
```

Not allowed by default:

```text
rename
delete
move
rewrite ZIP
extract into source
repair
recompress
```

## 32.2 Future rebuild mode

If later implemented:

```text
Source:
D:\Arcade\Original-ROMs\

Destination:
D:\Arcade\Rebuilt-FBNeo\
```

Every operation must be copy/rebuild-to-destination.

Provide dry-run manifest first.

---

# 33. Onboarding Wizard

First launch.

## Step 1 — Welcome

```text
Arcade ROM Router organizes mixed arcade ROM collections
and automatically selects a compatible configured emulator.

Your original ROM folder is read-only by default.
```

## Step 2 — ROM folder

```text
Choose folder
[ D:\Arcade\Original-ROMs ]
```

Options:

```text
[x] Scan subfolders
[x] Treat source as read-only
```

## Step 3 — Find RetroArch

```text
RetroArch found:
C:\RetroArch\retroarch.exe

[Use] [Choose another]
```

## Step 4 — Cores

```text
✓ FinalBurn Neo
✓ MAME 2003-Plus
✓ MAME 2010
○ MAME Current
```

A check means installed/discovered, not necessarily fully configured.

## Step 5 — DAT definitions

For each enabled profile:

```text
Core: MAME 2003-Plus
ROM definition: Missing
[Import DAT]
```

If the application can safely generate a definition from an installed emulator, offer it.

Do not download protected game data.

## Step 6 — Controller

```text
Xbox Wireless Controller detected
[Run test]
[Skip]
```

## Step 7 — First scan

```text
[Scan Library]
```

---

# 34. Emulator Manager

Screen sections:

```text
RetroArch
Executable
Config
Core directory
System directory
Status

Arcade Cores
Core
Installed
DAT
Health
Games matched
```

Example:

```text
FBNeo            ✓   ✓   Healthy    524
MAME 2003-Plus   ✓   ✓   Healthy    311
MAME 2010        ✓   ✓   Healthy     81
MAME Current     ✓   !   Needs DAT    —
```

---

# 35. DAT Manager UI

```text
MAME 2003-Plus
Definition: mame2003-plus.xml
Fingerprint: A81E...
Imported: 2026-08-09
Machines: 4,9xx

[Replace]
[Deactivate]
[Details]
```

Show warning before replacing active definition.

---

# 36. Problem Center

Aggregate problems into actionable groups:

```text
121 Missing dependencies
  72 parent sets
  31 BIOS sets
  18 device/other

63 Incomplete sets
19 Missing CHDs
27 Unidentified
5 Unreadable archives
```

Clicking a category filters games.

This is more useful than a generic error log.

---

# 37. “Can This Run?” Detail

Each game should answer:

```text
Can this run now?
YES / NO / MAYBE
```

With reason.

YES:

```text
Verified against MAME 2003-Plus definition.
All required ROM data and dependencies are present.
```

NO:

```text
Game data matches MAME 2010, but the required parent set is missing.
```

MAYBE:

```text
Several ROM chips are recognized by MAME, but the archive does not match
a complete known set in any active definition.
```

---

# 38. Automatic Routing Algorithm

Pseudo-code:

```text
function chooseRoute(archive):

    candidates = allMatchResults(archive)

    if userOverride exists:
        return validateOverride(userOverride)

    valid = candidates where
        result.contentComplete == true
        and result.dependenciesSatisfied == true
        and emulatorProfile.healthy == true
        and coreInstalled == true

    if valid is empty:
        return noLaunchableRoute(withBestDiagnostic(candidates))

    if only one valid:
        return valid[0]

    valid = applyGlobalPreferenceMode(valid)

    sort by:
        preferredCoreWeight desc
        verificationStrength desc
        dependencyComplexity asc
        emulatorHealth desc
        deterministicProfileId asc

    return first
```

Never select a route solely because a core is installed.

---

# 39. Matching Algorithm Pseudocode

```text
function matchArchive(archive, activeDats):

    members = inspectArchive(archive)

    candidates = set()

    filenameKey = normalizeSetName(archive.fileName)

    candidates += machinesWithSetName(filenameKey)

    for member in members:
        candidates += machinesContaining(member.crc32, member.size)

    for machine in candidates:
        expected = getExpectedLocalAndInheritedRequirements(machine)

        comparison = compare(members, expected.localArchiveRequirements)

        dependencies = resolveDependencies(machine)

        state = classify(comparison, dependencies)

        confidence = deriveConfidence(comparison, dependencies)

        persistMatchResult(
            archive,
            machine,
            state,
            confidence,
            comparison,
            dependencies
        )
```

Be careful with split, merged, and non-merged semantics.

The correct expected-local-content calculation depends on the DAT's relationship metadata. This logic needs dedicated tests.

---

# 40. Split / Merged / Non-Merged Awareness

The same logical game may be packaged differently.

Support classification metadata:

```text
SPLIT
MERGED
NON_MERGED
UNKNOWN_PACKAGING
```

Do not infer packaging mode globally from one archive.

Dependency resolution must understand whether required ROM content may legally be inherited from:

- parent set;
- BIOS set;
- device set;
- merged archive.

The match engine should keep a conceptual distinction between:

```text
logical machine requirements
```

and

```text
files expected physically inside this archive
```

This prevents false “missing ROM” errors.

---

# 41. CHD Indexing

Do not hash every CHD fully during a normal scan.

Index:

```text
path
file name
size
mtime
parent directory
```

Deep verify on demand.

If CHD tooling is available through installed MAME/chdman and support is implemented, use it through a safe adapter.

Never modify CHDs during verification.

---

# 42. Metadata and Artwork

Artwork must be optional.

The core product works without internet.

## 42.1 Local artwork folders

Support user-configurable conventions:

```text
artwork\
  box\
  screenshot\
  title\
  marquee\
  cabinet\
```

Lookup by:

1. canonical set name;
2. parent set name;
3. normalized title.

## 42.2 Online metadata

Future optional providers may supply:

- description;
- release year;
- manufacturer;
- genre;
- players;
- artwork.

Requirements:

- user explicitly enables provider;
- provider terms allow usage;
- cache responsibly;
- no ROM/BIOS retrieval;
- failure does not degrade core library functionality.

---

# 43. Security Model

This application handles untrusted local archives and launches external executables.

## 43.1 Archive safety

- never extract arbitrary paths to disk during scan;
- reject path traversal entries such as `..\`;
- enforce decompression size limits for any member that must be inflated;
- prefer metadata-only inspection;
- cap XML/DAT parsing resource use where reasonable;
- do not execute anything in archives.

## 43.2 Process safety

- executable paths must belong to validated emulator profiles;
- build argument arrays;
- never concatenate shell commands;
- never invoke `cmd /c` for normal launching;
- quote/path handling done by process API;
- do not allow ROM filename to inject arguments.

## 43.3 Database safety

- prepared statements;
- migrations;
- backup before destructive migration;
- recovery path if DB corrupts.

## 43.4 Network safety

MVP should require no network.

Optional update/metadata functions must:

- use HTTPS;
- identify domain explicitly;
- not transmit ROM contents or hashes without informed opt-in.

Do not upload the user's archive hashes by default.

---

# 44. Privacy

All library data is local by default.

Store in application data:

```text
settings
scan database
logs
artwork cache
DAT index
controller preferences
play history
```

Provide:

```text
[Clear play history]
[Clear cached artwork]
[Reset database]
[Export diagnostics]
```

A diagnostic export must have an option to redact absolute filesystem paths.

---

# 45. Legal / Content Boundaries

The app can organize and validate a user's legally obtained game data.

It must not include features whose purpose is to locate unauthorized copies of copyrighted ROMs.

Allowed:

```text
“Required BIOS set: neogeo”
“Missing parent: xyz”
“Expected CRC: 12345678”
```

Not built in:

```text
“Download missing ROM here”
torrent search
ROM website search
automatic copyrighted ROM acquisition
```

Links to official emulator documentation are acceptable.

---

# 46. Error Handling

Every error should be one of:

```text
user-actionable
configuration
content-validation
external-process
filesystem
database
internal
```

Do not dump raw Rust errors as the primary user message.

Example:

```text
Title:
Unable to read archive

Message:
The ZIP file could not be opened. The file may be damaged or incomplete.

File:
D:\Arcade\roms\abc.zip

Technical details:
<expandable>
```

---

# 47. Scan Job System

Represent long operations as jobs.

```text
QUEUED
RUNNING
PAUSED
CANCELLING
CANCELLED
COMPLETED
FAILED
```

Job types:

```text
FULL_SCAN
INCREMENTAL_SCAN
DEEP_VERIFY
DAT_IMPORT
DAT_REINDEX
EMULATOR_HEALTH_CHECK
EXPORT_REPORT
```

UI:

```text
Scanning arcade library
734 / 1,427
[Pause] [Cancel]
```

---

# 48. Route Health Checks

A route is only launchable if all required infrastructure is healthy.

RetroArch route:

```text
retroarch executable exists
core DLL exists
content archive exists
required DAT active
system path valid where necessary
required dependencies present
```

Standalone MAME route:

```text
mame executable exists
definition corresponds to installed MAME snapshot
content visible through launch/rompath strategy
dependencies present
```

---

# 49. Core Updates

Core updates can invalidate assumptions.

The Router should store a fingerprint of each core file:

```text
path
file size
modified time
sha256 optional
```

If changed:

```text
Core changed since last compatibility scan.
Re-verify the associated DAT before trusting routes.
```

Do not immediately mark every game broken.

Mark:

```text
ROUTE_REVERIFY_RECOMMENDED
```

until definition compatibility is confirmed.

---

# 50. Launch Logging

Per session:

```json
{
  "game": "sf2",
  "archivePath": "<redactable>",
  "emulatorProfile": "fbneo",
  "corePath": "<redactable>",
  "started": "ISO-8601",
  "ended": "ISO-8601",
  "exitCode": 0,
  "routerVersion": "...",
  "datFingerprint": "...",
  "diagnosticLog": "..."
}
```

Keep a bounded history.

---

# 51. User Experience for a Broken ROM

When user clicks a non-launchable game:

Do not show an inactive Play button with no explanation.

Instead:

```text
NOT READY TO PLAY

Your archive matches:
MAME 2010 — <game title>

What is wrong:
Required parent set is missing.

Required:
<parent set>

Available alternate routes:
MAME 2003-Plus — incomplete
FBNeo — no matching set

[View technical details]
[Rescan]
```

---

# 52. User Experience for Multiple Valid Routes

```text
3 compatible emulator routes found

Recommended:
FBNeo
Reason:
Complete checksum match, healthy core, all dependencies present.

Also compatible:
MAME Current
MAME 2003-Plus

[Use Recommended]
[Choose Route]
[Always use this route for this game]
```

---

# 53. Favorites / Recently Played

Simple local features.

Favorites:

```text
toggle instantly
filter library
controller shortcut
```

Recently played:

```text
sort by last played
play count
last selected route
```

Never let play-history data influence ROM verification.

---

# 54. Fullscreen / Cabinet Mode

Phase 2.

Features:

- launch app fullscreen;
- controller-only navigation;
- hidden mouse cursor after inactivity;
- large cards;
- overscan-safe layout;
- simple top-level categories;
- optional startup directly into library.

Exit must remain accessible.

Do not trap the user in fullscreen.

---

# 55. Keyboard Support

Default:

```text
Arrows       Navigate
Enter        Select/Play
Escape       Back
F            Favorite
Ctrl+F       Search
F5           Rescan selected / refresh
```

Avoid hijacking operating-system-reserved shortcuts.

---

# 56. Accessibility

- semantic controls;
- keyboard reachable;
- visible focus;
- no status conveyed by color alone;
- scalable text;
- reduced motion option;
- screen-reader labels on status icons;
- controller focus and keyboard focus use same logical system;
- high contrast mode.

---

# 57. Theme Tokens

Do not hard-code styling across components.

Example:

```css
--bg-0
--bg-1
--surface-1
--surface-2
--text-primary
--text-secondary
--accent
--success
--warning
--danger
--unknown
--focus
--radius-sm
--radius-md
--radius-lg
--space-1
...
```

Support future theme packs without changing business logic.

---

# 58. API Boundary Between React and Rust

Tauri commands should be narrow.

Examples:

```text
get_library_page
get_game_detail
start_scan
cancel_scan
get_scan_status
list_emulator_profiles
detect_retroarch
validate_emulator_profile
import_dat
list_dat_sources
choose_route
set_game_route_override
launch_game
get_controller_settings
export_diagnostics
```

Do not expose generic:

```text
run_process(commandString)
read_any_file(path)
write_any_file(path)
```

to frontend.

---

# 59. Rust Domain Types

Examples:

```rust
enum CompatibilityState {
    VerifiedPlayable,
    MissingParent,
    MissingBios,
    MissingDevice,
    MissingChd,
    IncompleteSet,
    WrongRomRevision,
    Ambiguous,
    ArchiveUnreadable,
    Unidentified,
}

enum Confidence {
    Verified,
    Strong,
    Partial,
    Unknown,
}

enum RunnerType {
    RetroArch,
    StandaloneMame,
}

struct RomHash {
    crc32: Option<String>,
    sha1: Option<String>,
}
```

Use enums in backend and serialized stable values at boundary.

---

# 60. Tests

Testing is mandatory because ROM matching is easy to get subtly wrong.

## 60.1 Unit tests

- normalize archive filename;
- ZIP member enumeration;
- CRC parsing;
- DAT XML parsing;
- clone relationship parsing;
- BIOS relationship parsing;
- required/optional ROM distinction;
- candidate generation;
- exact set match;
- incomplete set;
- wrong CRC;
- missing parent;
- missing BIOS;
- CHD required;
- multiple valid route ranking;
- user override;
- argument escaping/path handling.

## 60.2 Fixture policy

Do not use copyrighted ROM data.

Create synthetic archives such as:

```text
test-parent.zip
test-clone.zip
test-bios.zip
```

with random/generated byte payloads and a matching synthetic DAT.

Example:

```text
parent-a.bin -> generated fixture bytes
parent-b.bin -> generated fixture bytes
clone-c.bin  -> generated fixture bytes
```

Compute known CRCs during fixture generation.

## 60.3 Integration tests

- scan synthetic directory;
- import synthetic DAT;
- produce expected compatibility states;
- route correct core;
- ensure source files unchanged;
- simulate missing parent;
- simulate missing BIOS;
- simulate corrupted ZIP;
- simulate core missing;
- simulate core update invalidation.

## 60.4 Golden parser tests

Store sanitized sample outputs from:

- MAME version/help;
- MAME `-verifyroms`;
- MAME `-romident`.

Parser tests must tolerate irrelevant whitespace changes where safe.

---

# 61. Acceptance Tests

## Scenario A — Old valid set

Given:

```text
game.zip
```

matches MAME 2003-Plus DAT exactly.

Expected:

```text
VERIFIED_PLAYABLE
route = MAME 2003-Plus
confidence = Verified
```

## Scenario B — Modern valid set

Archive matches current MAME definition.

Expected:

```text
MAME Current route is available.
```

## Scenario C — Mixed library

Folder contains archives matching three different cores.

Expected:

```text
one library
individual route per game
no manual folder splitting required
```

## Scenario D — Missing parent

Clone archive is valid locally but parent is absent.

Expected:

```text
MISSING_PARENT
not launchable by default
exact parent name shown
```

## Scenario E — Missing BIOS

Expected:

```text
MISSING_BIOS
game data itself marked as matched
BIOS dependency shown separately
```

## Scenario F — Unknown archive

Expected:

```text
UNIDENTIFIED
no automatic route
```

## Scenario G — Renamed archive

Members perfectly match a known set, ZIP name differs.

Expected:

```text
strong/verified content identification
show expected canonical set name
do not rename source automatically
```

## Scenario H — Multiple routes

Expected:

```text
all verified routes retained
one selected deterministically
user can override
```

## Scenario I — Source safety

After scan, compare hashes of all fixture archives.

Expected:

```text
no source file changed
```

---

# 62. MVP Scope

MVP is complete when a Windows user can:

1. install/open Arcade ROM Router;
2. choose an original arcade ROM folder;
3. choose/detect RetroArch;
4. detect installed arcade cores;
5. import DAT definitions;
6. scan ZIP ROM sets;
7. see identification and compatibility state;
8. see missing parent/BIOS/CHD diagnostics;
9. see multiple possible routes;
10. have Router choose a verified route;
11. click Play;
12. have RetroArch launch using the selected core;
13. navigate the library with keyboard and common controllers;
14. favorite games;
15. filter by working/problem/core;
16. rescan incrementally;
17. export a compatibility report;
18. leave original ROM files untouched.

If these are not all working, MVP is not finished.

---

# 63. Implementation Phases

## Phase 0 — Repository Foundation

Deliver:

```text
Tauri shell
React UI
SQLite migrations
structured logging
settings storage
PROGRESS.md
test harness
```

Exit criteria:

- application starts;
- backend command works;
- database migration works;
- test suite runs.

## Phase 1 — ROM Inventory

Deliver:

```text
ROM root selection
ZIP enumeration
archive member inspection
CRC/size capture
incremental cache
scan progress
```

UI:

```text
filename
archive status
member count
```

No emulator routing yet.

## Phase 2 — DAT Import

Deliver:

```text
XML DAT parser
DAT database tables
machine/member indexes
DAT manager UI
fingerprints
```

Verify with synthetic DAT.

## Phase 3 — Matching Engine

Deliver:

```text
candidate generation
exact content matching
incomplete detection
renamed archive detection
confidence states
```

At this point, app should accurately say what each archive most likely is.

## Phase 4 — Dependencies

Deliver:

```text
parent/clone
BIOS
device
CHD
optional samples
```

Problem Center becomes functional.

## Phase 5 — Emulator Profiles

Deliver:

```text
RetroArch executable discovery
core discovery
profile editor
health checks
DAT association
```

## Phase 6 — Router

Deliver:

```text
valid route generation
route ranking
preference modes
user overrides
```

No launch until route tests pass.

## Phase 7 — RetroArch Launch

Deliver:

```text
safe process spawn
explicit -L core
content path
launch history
verbose diagnostic mode
exit status
```

## Phase 8 — Controller Center

Deliver:

```text
Gamepad API navigation
controller test screen
focus model
big-card navigation
RetroArch controller-profile status guidance
```

## Phase 9 — Polished Library

Deliver:

```text
game grid
list mode
details
filters
favorites
recent
search
status cards
empty/loading/error states
```

## Phase 10 — MAME Secondary Verification

Deliver:

```text
standalone MAME adapter
-version
-listxml import
-verifyroms
-romident
```

Keep this supplemental to DAT matching.

## Phase 11 — Performance / Hardening

Deliver:

```text
large-library benchmark
bounded parallelism
memory audit
malformed ZIP tests
path-injection tests
DB backup/recovery
log rotation
```

## Phase 12 — Packaging

Deliver:

```text
Windows installer
portable option if feasible
first-run onboarding
release build
version screen
diagnostic export
```

---

# 64. Suggested First Development Milestone

Cursor should implement only:

```text
Phase 0
+
Phase 1
```

before attempting core routing.

First usable screen:

```text
ARCADE ROM ROUTER

ROM folder:
D:\Arcade\Original-ROMs

1,427 archives

Name          Members     CRC indexed     State
1942.zip       14           Yes            Indexed
sf2.zip        21           Yes            Indexed
abc.zip         —           —              Unreadable
```

This establishes safe scanning infrastructure.

---

# 65. `PROGRESS.md` Format

Cursor must keep this file current.

```markdown
# Arcade ROM Router Progress

## Current phase
Phase 2 — DAT Import

## Completed
- [x] Tauri shell
- [x] SQLite migrations
- [x] ROM root selector
- [x] ZIP member scanner

## In progress
- [ ] DAT XML parser

## Next
- [ ] machine import
- [ ] ROM CRC index
- [ ] DAT manager UI

## Decisions
- 2026-08-09: Source ROM roots remain read-only by default.
- 2026-08-09: DAT matching is primary; brute-force emulator launching rejected.

## Known issues
- Large ZIP scan cancellation needs testing.

## Tests
- 42 passing
- 0 failing
```

---

# 66. Export Formats

## Compatibility CSV

Columns:

```text
file
set_name
title
status
confidence
selected_emulator
alternate_emulators
parent
bios
chd_required
missing_items
last_scanned
```

## Diagnostic JSON

Full structured evidence.

## Human-readable Markdown

Example:

```markdown
# Arcade ROM Compatibility Report

## Summary
- 1,427 scanned
- 1,103 playable
- 121 missing dependencies

## Problems

### game.zip
Status: Missing BIOS
Matched route: FBNeo
Required BIOS: <name>
```

Redact absolute paths optionally.

---

# 67. Settings

## Library

```text
ROM roots
recursive scan
file watcher
artwork folders
```

## Matching

```text
preference mode
deep verification
hashing level
historical cores enabled
```

## Emulators

```text
RetroArch path
core directory
standalone MAME path
system directories
```

## Controls

```text
controller navigation
keyboard shortcuts
menu behavior
```

## Appearance

```text
theme
card size
grid density
reduced motion
fullscreen mode
```

## Privacy

```text
play history
log retention
path redaction
network metadata providers
```

---

# 68. Smart Rescan

When user clicks:

```text
Rescan
```

default to incremental.

Separate:

```text
Quick Rescan
Full Rescan
Deep Verify
```

Definitions:

### Quick Rescan
metadata changes only.

### Full Rescan
re-inspect all archives but reuse DAT indexes.

### Deep Verify
full archive validation plus stronger hashes/external verifier where available.

---

# 69. Duplicate Detection

Optional feature.

Detect:

```text
exact same archive SHA-256
same ROM member checksum set
same canonical machine match
```

Do not automatically delete duplicates.

UI:

```text
Possible duplicate
2 files represent the same verified set.

[Compare]
```

---

# 70. Canonical Identity

Do not use filename as the permanent game identity.

Logical identity should be:

```text
DAT source + machine set name
```

Local content identity:

```text
archive record
```

A single logical game may have:

- multiple local archives;
- multiple DAT machine definitions;
- multiple emulator routes.

The DB must preserve these distinctions.

---

# 71. Parent/Clone Display Preference

By default:

```text
Show all games
```

Optional:

```text
Group clones under parent
```

Grouped UI:

```text
Street Fighter II
  World
  USA
  Japan
  Revision ...
```

Do not hide clones automatically.

---

# 72. Working-State Semantics

Do not use “Working” to mean only “emulator process launched.”

Use:

```text
Verified playable
```

for deterministic content/dependency state.

Optionally add:

```text
User tested ✓
```

after successful confirmation.

These are separate.

---

# 73. User Feedback Loop

After first successful launch of a route:

```text
Did it work correctly?

[Yes]
[Audio/video issue]
[Controls issue]
[Failed to boot]
[Don't ask again]
```

Store separately from checksum compatibility.

This can inform UI:

```text
Verified content
User tested
```

or:

```text
Verified content
User reported controls issue
```

Do not downgrade ROM correctness merely due to controller trouble.

---

# 74. Troubleshooting Assistant Within App

Not AI-dependent.

Rule-based explanations:

```text
If MISSING_PARENT:
 explain parent/clone relationship.

If MISSING_BIOS:
 explain BIOS dependency.

If WRONG_ROM_REVISION:
 explain that same game name can have different expected chip dumps across emulator versions.

If UNIDENTIFIED:
 suggest importing additional DATs or running MAME identification if configured.

If CORE_NOT_INSTALLED:
 direct user to Emulator Manager.

If CONTROLS issue:
 direct to Controller Center / RetroArch remap.
```

---

# 75. Advanced: Definition Coverage View

Useful for mixed collections.

```text
Your collection coverage

FBNeo            524 exact
MAME Current     431 exact
MAME 2003-Plus   311 exact
MAME 2010        112 exact
```

These counts may overlap.

Then:

```text
Unique additional playable games contributed by profile

FBNeo             +402
MAME Current      +331
MAME 2003-Plus    +207
MAME 2010          +41
```

This helps user decide which historical cores are actually useful.

---

# 76. Advanced: Route Provenance

Display:

```text
Route:
MAME 2003-Plus

Selected because:
1. All 14 required ROM members match.
2. Required parent is present.
3. No CHD required.
4. Core passed health check.
5. This is the highest-priority valid route under “Maximum Legacy Compatibility.”
```

Every automatic decision should be explainable like this.

---

# 77. Advanced: Core Retirement

If user disables a core:

- do not delete match history;
- mark its routes unavailable;
- reroute games with alternate valid routes;
- show games that become stranded.

Example:

```text
Disabling MAME 2010 will make 17 games currently unlaunchable.

[Review games]
[Cancel]
[Disable]
```

---

# 78. Advanced: DAT Update Impact Preview

Before activating a replacement DAT:

```text
DAT update impact

1,427 local archives evaluated

1,308 unchanged
62 gain a verified match
31 lose current verification
18 change canonical set
8 require review

[View changes]
[Activate]
```

Can be Phase 3+.

---

# 79. Advanced: Optional Core Benchmark

Do not benchmark automatically.

For a game with several verified routes:

```text
[Compare Routes]
```

Could collect:

```text
launch success
average emulation speed if log exposes it
user control preference
```

But never promote a route solely because it starts faster if accuracy/compatibility is uncertain.

---

# 80. Future Rebuild Assistant

Not MVP.

If implemented, it should use DAT definitions to create a **new** validated collection.

Flow:

```text
Choose target:
FBNeo current

Source ingredients:
D:\Arcade\Original-ROMs

Destination:
D:\Arcade\Rebuilt-FBNeo

Dry run:
524 complete sets can be built
67 need additional data
836 not applicable

[Export Plan]
[Build to Destination]
```

Never fetch missing protected data.

Never alter source.

This could eventually replace much manual ClrMamePro work for this user's workflow, but only after the scanner/router is mature.

---

# 81. Future “One Library” Abstraction

The long-term app should hide emulator fragmentation from normal use.

Normal screen:

```text
PAC-MAN
[PLAY]
```

Advanced screen:

```text
Selected emulator:
MAME 2003-Plus

Why:
Exact ROM definition match
```

Technical complexity belongs in diagnostics, not the main Play flow.

---

# 82. Reference Emulator Facts Used by This Specification

These are implementation-relevant facts that should be revalidated against official documentation whenever emulator integration is changed.

1. Arcade emulator cores expect specific ROM-set definitions; mixing arbitrary historical sets and cores is unreliable.
2. MAME 2003 corresponds to MAME 0.78 ROM sets.
3. MAME 2003-Plus began from MAME 0.78 drivers, with additional updated/new definitions.
4. MAME 2010 corresponds to MAME 0.139 ROM sets.
5. Historical Libretro MAME cores exist for 2015 and 2016 generations.
6. FBNeo also requires matching ROM sets and can use a DAT to define them.
7. MAME supports ROM verification and ROM identification command-line operations.
8. RetroArch can explicitly launch content with a specified libretro core using `-L`.
9. RetroArch has a controller autoconfiguration system.
10. Some arcade cores support specialized control behavior, such as MAME 2003-Plus 4-way joystick simulation.
11. CHD-based games require disk data in addition to ROM ZIP data.
12. Parent, BIOS, device, and sample dependencies must be distinguished.

---

# 83. Official Technical References

Use official project documentation as the first source for implementation questions.

## MAME

MAME command-line documentation:

https://docs.mamedev.org/commandline/commandline-all.html

MAME command-line index:

https://docs.mamedev.org/commandline/commandline-index.html

MAME documentation home:

https://docs.mamedev.org/

## Libretro / RetroArch

Getting Started with Arcade Emulation:

https://docs.libretro.com/guides/arcade-getting-started/

RetroArch Command-Line Interface:

https://docs.libretro.com/guides/cli-intro/

Controller Auto-Configuration:

https://docs.libretro.com/guides/controller-autoconfiguration/

Input and Controls:

https://docs.libretro.com/guides/input-and-controls/

MAME 2003:

https://docs.libretro.com/library/mame_2003/

MAME 2003-Plus:

https://docs.libretro.com/library/mame2003_plus/

MAME 2010:

https://docs.libretro.com/library/mame_2010/

FinalBurn Neo:

https://docs.libretro.com/library/fbneo/

RetroArch core list:

https://docs.libretro.com/guides/core-list/

---

# 84. README Opening Draft

Use this or a polished equivalent.

```markdown
# Arcade ROM Router

Arcade ROM Router is a local desktop library for mixed historical arcade ROM collections.

Instead of forcing every ROM through one MAME version, it inventories each archive, compares its ROM-chip checksums against emulator-specific DAT definitions, identifies missing parent/BIOS/CHD dependencies, and chooses a verified installed emulator route automatically.

The normal experience is simple:

1. Choose your arcade ROM folder.
2. Configure RetroArch and the arcade cores you use.
3. Import matching DAT definitions.
4. Scan.
5. Pick a game.
6. Press Play.

Your original ROM directory is read-only by default.

Arcade ROM Router does not include or download copyrighted ROMs, BIOS files, or CHDs.
```

---

# 85. Cursor Bootstrap Prompt

Paste the following into Cursor after placing this file in the new repository:

```text
You are implementing Arcade ROM Router.

Read SPEC.md completely before writing architecture code.

Create and maintain PROGRESS.md.

Begin with Phase 0 and Phase 1 only:
- Tauri 2 desktop shell
- React + TypeScript frontend
- Rust backend
- SQLite migrations
- structured logging
- settings
- ROM root selection
- safe read-only ZIP enumeration
- ZIP member filename/size/CRC indexing
- cancellable scan jobs
- incremental scan cache
- basic library table
- synthetic fixture tests

Do NOT implement emulator routing yet.
Do NOT download ROMs, BIOS files, CHDs, or copyrighted game data.
Do NOT modify files in the selected ROM directory.
Do NOT execute any file discovered in a ROM directory.
Use safe native process and filesystem APIs.
Use synthetic non-copyrighted test archives.

After Phase 1:
1. update PROGRESS.md;
2. run tests;
3. document architecture decisions;
4. identify any deviations from SPEC.md;
5. stop before Phase 2 unless explicitly instructed to continue.

The first objective is a trustworthy ROM inventory engine.
```

---

# 86. Phase 2 Cursor Prompt

After Phase 1 is verified:

```text
Continue Arcade ROM Router using SPEC.md as source of truth.

Implement Phase 2 — DAT Import.

Requirements:
- XML DAT import
- source fingerprinting
- machines table
- machine_roms table
- machine_disks table
- clone/rom-of metadata
- required vs optional ROM records
- CRC32, size, SHA-1 indexes
- import progress
- duplicate DAT detection
- DAT Manager UI
- synthetic DAT test suite

Do not route or launch games yet.

When complete:
- update PROGRESS.md
- run all tests
- report database migration changes
- report supported DAT constructs
- list unsupported constructs explicitly
```

---

# 87. Phase 3–7 Cursor Prompt

After DAT import is stable:

```text
Continue Arcade ROM Router according to SPEC.md.

Implement Phases 3 through 7 incrementally, with tests at each boundary:

Phase 3: matching engine
Phase 4: dependency resolution
Phase 5: emulator profiles and RetroArch/core discovery
Phase 6: automatic route selection
Phase 7: safe RetroArch launch

Critical rules:
- checksum/DAT evidence is primary
- do not brute-force launch ROMs through every core
- never silently modify source ROMs
- never execute via a shell-concatenated command
- use explicit process argument arrays
- retain evidence for every match and route
- user overrides are explicit and reversible
- non-verified routes do not auto-launch by default

Do not skip tests for split/clone/BIOS cases.

Update PROGRESS.md after each phase.
```

---

# 88. Definition of Success

The project is successful when the user can place decades of mixed arcade ROM archives in one source folder and interact with them as a single library while the application transparently handles emulator-version fragmentation.

The user should eventually experience:

```text
Open Arcade ROM Router
        ↓
Search “Galaga”
        ↓
Select game
        ↓
PLAY
        ↓
Correct compatible emulator/core is launched
```

and when a game cannot run:

```text
Select game
        ↓
NOT READY
        ↓
Clear explanation:
“Your game data matches MAME 2010, but the required parent set is missing.”
```

The application should transform MAME-version confusion into a visible, deterministic compatibility system.

That is the product.

---

# 89. Final Architectural Principle

**Do not build a frontend that happens to launch MAME.**

Build a **ROM compatibility intelligence layer** with a frontend on top of it.

The central domain model is:

```text
LOCAL CONTENT
    ↓
CHECKSUM EVIDENCE
    ↓
ROM DEFINITION / DAT
    ↓
LOGICAL ARCADE MACHINE
    ↓
DEPENDENCY GRAPH
    ↓
VALID EMULATOR ROUTES
    ↓
ROUTING POLICY
    ↓
RETROARCH / MAME LAUNCH
```

If this chain is implemented correctly, the user can keep a heterogeneous old ROM collection while the Router handles the historical emulator distinctions automatically.
