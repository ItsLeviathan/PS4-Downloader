# PS4 Downloader — PC Application Development Prompt

## Role

You are Claude Code working directly inside:

```text
F:\PS4 Downloader
```

Build a complete, polished Windows desktop application for downloading **user-authorized files** and organizing large game files on a dedicated storage drive.

The application will primarily use:

```text
F:\PS4 Games
```

as its default storage location.

Do not assume the repository is empty. **Inspect the existing directory first** and preserve useful existing files unless they are clearly obsolete.

---

# 1. Main Objective

Create a modern Windows desktop download manager designed for large files.

The application should provide:

* Fast downloading
* Resumable downloads
* Download queue
* Multiple simultaneous downloads
* Download progress
* Download speed
* ETA
* Pause/resume
* Cancel
* Retry
* Automatic filename detection
* Storage management
* Download history
* File integrity verification when checksums are available
* Large-file support
* Dedicated HDD storage
* Clean modern UI
* Reliable error handling

The application should feel like a polished standalone Windows application, not a prototype.

---

# 2. Important Legal / Security Boundary

The application is a **general-purpose downloader for files the user is authorized to download**.

Do NOT implement mechanisms intended to bypass:

* CAPTCHA
* DRM
* authentication
* paywalls
* anti-bot protections
* rate limits
* access controls
* download restrictions
* security mechanisms
* protected/private content

Do not attempt to circumvent website protections.

If a server requires authentication or a browser session, clearly report that the URL cannot be downloaded directly rather than attempting to bypass the protection.

The downloader may follow normal HTTP/HTTPS redirects and use standard HTTP functionality.

---

# 3. Recommended Technology

Use:

### Desktop Framework

**Tauri**

### Frontend

**React + TypeScript**

### Backend / Native Layer

**Rust**

### Styling

Use a modern CSS solution appropriate for the chosen React setup.

Prefer a clean component architecture.

Avoid unnecessary dependencies.

Before installing packages, inspect the existing project configuration.

---

# 4. First Action — Inspect the Project

Before modifying anything:

1. Inspect the entire repository.
2. Identify whether a Tauri project already exists.
3. Inspect:

   * `package.json`
   * `src`
   * `src-tauri`
   * configuration files
   * lockfiles
   * README
   * Git status
4. Determine the current state of the project.
5. Reuse existing configuration when appropriate.
6. Do not delete or overwrite existing work unnecessarily.

Then create an implementation plan.

After the plan is understood, begin implementation.

---

# 5. Application Name

Use:

**PS4 Downloader**

Possible internal project identifier:

```text
ps4-downloader
```

The application should be designed so the name can easily be changed later.

---

# 6. Default Storage Location

Default game/file storage:

```text
F:\PS4 Games
```

The application must NOT assume that `F:` will always exist.

On startup:

1. Check whether `F:\PS4 Games` exists.
2. If it exists, use it.
3. If it doesn't exist, show a clear warning.
4. Allow the user to choose another folder.
5. Remember the selected location.

Never crash simply because the F: drive is unavailable.

---

# 7. Storage Structure

Create the following directory structure when needed:

```text
F:\PS4 Games\
├── Downloads\
├── Completed\
├── Incomplete\
└── Metadata\
```

### Downloads

Temporary active download workspace.

### Completed

Finished files.

### Incomplete

Partially downloaded files that can be resumed.

### Metadata

Application metadata only.

Do not store unnecessary copies of downloaded files.

---

# 8. Main UI

Create a modern desktop interface.

The visual direction should be:

* Dark
* Minimal
* Premium
* Clean
* Modern
* Easy to understand
* Large readable typography
* Subtle animations
* No excessive gradients
* No clutter
* No unnecessary decorative elements

The application should feel closer to a modern download manager than a developer tool.

---

# 9. Main Dashboard

The primary screen should contain:

```text
PS4 Downloader

[ Paste Download URL                         ]

                         [ Download ]

Storage
F:\PS4 Games

Free Space:       XX GB
Used Space:       XX GB
Total Space:      XX GB

Downloads
────────────────────────────────────────────

Game/File Name
████████████████████░░░░░░  74%

12.4 MB/s
4.2 GB / 5.7 GB
ETA 00:12

[ Pause ] [ Cancel ]

────────────────────────────────────────────
```

Use cards/components rather than a cluttered table.

---

# 10. URL Input

The user should be able to paste a normal HTTP/HTTPS download URL.

Example:

```text
https://example.com/file.zip
```

When the user presses Download:

1. Validate the URL.
2. Check whether HTTP/HTTPS is supported.
3. Perform a metadata request where appropriate.
4. Determine filename.
5. Determine file size when available.
6. Determine whether the server supports range requests.
7. Add the item to the queue.
8. Start downloading.

Do not download anything merely because the URL was pasted.

---

# 11. Filename Detection

Attempt to determine the filename using:

1. `Content-Disposition`
2. URL filename
3. Server-provided metadata

Sanitize filenames for Windows.

Prevent invalid Windows characters.

Prevent path traversal.

Never allow a remote URL to specify an arbitrary filesystem path.

All downloaded files must remain inside the configured storage directory.

---

# 12. Download Engine

The Rust backend should handle downloading.

Implement:

### Standard download

If the server does not support HTTP ranges:

```text
GET file
↓
write sequentially
```

### Resumable download

If the server supports HTTP range requests:

```text
Range: bytes=start-
```

allow the download to resume.

Store incomplete downloads safely.

Do not restart an existing partial download unnecessarily.

---

# 13. Parallel Downloading

Implement segmented downloading only when the server supports HTTP range requests.

For example:

```text
File
│
├── Segment 1
├── Segment 2
├── Segment 3
└── Segment 4
```

Download segments concurrently and combine them safely.

Do NOT assume that more connections always means faster downloads.

Provide a configurable maximum number of connections.

Default:

```text
4 connections
```

Allow:

```text
1
2
4
8
```

Avoid aggressive connection counts.

Respect normal server behavior and avoid overwhelming servers.

---

# 14. Download Speed

Display:

```text
12.4 MB/s
```

Calculate speed over a rolling interval rather than displaying unstable instantaneous values.

Also display:

```text
Downloaded
Total
Percentage
ETA
```

Example:

```text
74%

4.2 GB / 5.7 GB

12.4 MB/s

ETA 00:12
```

---

# 15. Pause / Resume

Every active download should support:

```text
Pause
Resume
Cancel
```

When paused:

* preserve partial data
* preserve metadata
* do not delete the incomplete file

When resumed:

* continue from the existing data
* verify the local partial state
* use HTTP range requests when supported

---

# 16. Download Queue

Create a queue system.

Example:

```text
Downloads

1. Game A.zip       Downloading
2. Game B.zip       Waiting
3. Game C.zip       Waiting
4. Game D.zip       Waiting
```

Allow:

* Start
* Pause
* Resume
* Cancel
* Remove
* Retry failed downloads

Provide a configurable maximum number of simultaneous downloads.

Default:

```text
2 simultaneous downloads
```

---

# 17. Large File Support

The application must be designed for very large files.

Do not load entire files into memory.

Use streaming I/O.

Use appropriate integer types for large file sizes.

Support files larger than 4 GB.

Do not use logic that assumes file sizes fit into 32-bit integers.

---

# 18. Disk Space Validation

Before starting a download:

1. Determine expected file size when available.
2. Check available disk space.
3. Warn if insufficient space exists.

Example:

```text
Not enough storage space.

Required:
86.4 GB

Available:
52.1 GB

Choose another storage location or free up space.
```

Do not begin a download that obviously cannot fit.

---

# 19. HDD Detection

Because the application uses:

```text
F:\PS4 Games
```

detect:

* drive availability
* total capacity
* free space
* used space

If the drive disappears while downloading:

```text
Storage drive unavailable.

The download has been paused.

Reconnect the drive to continue.
```

Do not corrupt the partial file.

---

# 20. Download Completion

When a download completes:

1. Flush data.
2. Close the file.
3. Verify expected size.
4. Optionally calculate checksum.
5. Move the completed file from:

```text
Incomplete
```

to:

```text
Completed
```

Use safe file operations.

Avoid leaving corrupted files marked as completed.

---

# 21. Checksum Verification

If the source provides a checksum, support:

* SHA-256
* SHA-1
* MD5

Prefer SHA-256.

The UI should display:

```text
Integrity

✓ Verified
```

or:

```text
✕ Verification failed
```

Do not claim a file is verified unless an actual checksum comparison was performed.

---

# 22. Download History

Create a History screen.

Display:

```text
Completed
Failed
Cancelled
```

Example:

```text
File Name
Size
Date
Status
Location
```

Allow the user to:

* Open containing folder
* Remove history entry
* Retry failed download

Removing history must NOT automatically delete the actual downloaded file unless the user explicitly chooses a delete action.

---

# 23. Notifications

Provide desktop notifications for:

### Completed

```text
Download completed

Game/File.zip
5.7 GB
```

### Failed

```text
Download failed

Game/File.zip
Connection error
```

### Storage problem

```text
Download paused

Storage drive unavailable.
```

Allow notifications to be disabled in Settings.

---

# 24. Settings

Create a Settings screen.

Include:

### Storage

```text
Download location
F:\PS4 Games

[ Change ]
```

### Download

```text
Simultaneous downloads: 2

Connections per download: 4

[ ] Start downloads automatically
```

### Notifications

```text
[✓] Download completed
[✓] Download failed
[✓] Storage warnings
```

### Appearance

```text
Dark
Light
System
```

### Startup

Only include startup behavior if it is implemented properly.

Do not add fake settings.

---

# 25. Security

The application must safely handle arbitrary URLs.

Protect against:

* path traversal
* invalid filenames
* malformed URLs
* unsupported protocols
* unsafe filesystem paths
* filename injection
* corrupted metadata
* unexpected redirects

Only allow:

```text
http://
https://
```

unless another protocol is explicitly implemented and justified.

Do not execute downloaded files automatically.

Do not launch downloaded installers automatically.

Do not execute downloaded scripts.

---

# 26. Error Handling

Errors must be understandable to normal users.

Avoid exposing raw Rust stack traces in the UI.

Instead of:

```text
reqwest::Error...
```

show:

```text
Unable to connect to the server.

Check your internet connection and try again.
```

For technical details, provide:

```text
View details
```

---

# 27. Network Failures

If the connection temporarily fails:

* retry automatically
* use exponential backoff
* limit retry attempts
* preserve partial data

Example:

```text
Connection lost.

Retrying in 5 seconds...
Attempt 2/5
```

Do not retry indefinitely.

---

# 28. Application State

Downloads must survive application restarts.

If the application closes while a download is active:

```text
Application closed
        ↓
Partial file preserved
        ↓
Application reopened
        ↓
Download detected
        ↓
Resume available
```

On startup, detect incomplete downloads and display:

```text
Incomplete Downloads

Game A.zip
74%

[ Resume All ]
```

---

# 29. File Manager Integration

For completed downloads, provide:

```text
Open Folder
```

This should open the appropriate Windows Explorer directory.

Also allow:

```text
Open File Location
```

Do not execute the downloaded file.

---

# 30. UI Navigation

Use a simple sidebar or top navigation.

Recommended:

```text
Dashboard
Downloads
Completed
History
Storage
Settings
```

Keep navigation minimal.

---

# 31. Dashboard Storage Card

Display the current storage drive:

```text
F:\PS4 Games

████████████░░░░░░

1.82 TB free
3.18 TB used
5.00 TB total
```

Use actual filesystem information.

Never hard-code these values.

---

# 32. Dark Mode

Dark mode should be the default.

Use:

* dark background
* slightly lighter cards
* clear typography
* subtle borders
* restrained accent color
* readable progress indicators

Avoid making every element glow.

The UI should look professional.

---

# 33. Performance

The application must remain responsive while downloading large files.

Do not:

* block the UI thread
* load large files into React state
* repeatedly render huge amounts of download data
* calculate expensive checksums on the UI thread

The Rust backend should perform:

* network operations
* file I/O
* checksum calculations
* download management

The frontend should primarily handle:

* presentation
* user interaction
* state display

---

# 34. Architecture

Prefer this general architecture:

```text
React / TypeScript
        │
        │ Tauri Commands / Events
        ▼
Rust Backend
        │
        ├── Download Manager
        ├── HTTP Client
        ├── Segment Manager
        ├── File Manager
        ├── Storage Manager
        ├── Checksum Manager
        └── Persistent State
```

Use events to communicate download progress:

```text
Rust
 ↓
download-progress
 ↓
React UI
```

Do not poll excessively.

---

# 35. Persistence

Persist:

* download queue
* incomplete downloads
* completed history
* settings
* storage location

Use an appropriate lightweight local storage/database solution.

Do not introduce a remote backend.

This application should work locally.

---

# 36. No Account System

Do not create:

* login
* registration
* cloud account
* subscription
* remote database

The application is a local Windows utility.

---

# 37. No Telemetry

Do not add analytics or tracking.

Do not send download URLs to an external service.

Do not collect user files.

Do not collect personal information.

---

# 38. Testing

Before declaring the project complete, test:

### Basic

* Application starts
* Dashboard renders
* Settings work
* Storage location works

### Download

* Small file
* Large file
* Redirected URL
* Server with range support
* Server without range support

### Resume

* Pause
* Resume
* Close application
* Reopen application
* Continue partial download

### Storage

* F: available
* F: unavailable
* Insufficient disk space
* Drive disconnected during download

### Errors

* Invalid URL
* Connection failure
* HTTP 404
* HTTP 403
* Server unavailable
* Corrupted partial file

### Security

* Invalid filenames
* Path traversal attempts
* Unsupported protocols

---

# 39. Build Requirements

The application must eventually produce a Windows executable.

Before finalizing:

```text
npm run build
```

or the appropriate project build command.

Then build the Tauri application.

Verify that the resulting Windows application launches successfully.

Do not claim a build succeeded unless you actually ran it.

---

# 40. Code Quality

Follow these principles:

* TypeScript strictness where practical
* Rust error handling
* No unnecessary `unwrap()` for recoverable runtime errors
* Small reusable React components
* Clear naming
* No duplicated logic
* No dead code
* No placeholder features presented as finished
* No fake statistics
* No fake download progress
* No hard-coded storage capacity
* No hard-coded download speeds

---

# 41. README

Create/update:

```text
README.md
```

Document:

* Project purpose
* Features
* Tech stack
* Development setup
* Running the app
* Building the app
* Storage structure
* Download architecture
* Security considerations
* Known limitations

Do not include claims about bypassing website restrictions.

---

# 42. Development Workflow

Follow this workflow:

```text
1. Inspect repository
2. Plan architecture
3. Install only necessary dependencies
4. Build application shell
5. Build storage manager
6. Build download engine
7. Build download queue
8. Build persistence
9. Build UI
10. Add error handling
11. Add notifications
12. Test
13. Fix issues
14. Build Windows application
15. Update README
```

Do not attempt to implement everything blindly in one huge file.

Keep the project modular.

---

# 43. Important Instruction About Existing Code

If existing code is present:

* inspect it first
* preserve useful work
* refactor when appropriate
* do not delete the project just to start over
* do not replace working functionality without understanding it

If the existing project is incomplete, improve it incrementally.

---

# 44. Final Deliverable

At the end, the repository should contain a working Windows desktop downloader.

Expected structure should be approximately:

```text
F:\PS4 Downloader\
│
├── src\
│   ├── components\
│   ├── pages\
│   ├── hooks\
│   ├── stores\
│   ├── types\
│   └── ...
│
├── src-tauri\
│   ├── src\
│   │   ├── downloader\
│   │   ├── storage\
│   │   ├── checksum\
│   │   ├── state\
│   │   └── ...
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── package.json
├── README.md
└── ...
```

The exact structure may differ if there is a better architecture.

---

# 45. Claude Code Behavior

Do not continuously ask me for confirmation for routine implementation decisions.

Make reasonable engineering decisions yourself.

However, stop and ask before:

* deleting substantial existing code
* changing the core framework
* introducing a paid service
* introducing a remote backend
* implementing anything intended to bypass access controls
* making destructive filesystem operations outside the application directory

For normal implementation choices, proceed autonomously.

---

# 46. Final Response After Implementation

When finished, provide a concise summary containing:

### Implemented

List the major completed features.

### Files Changed

List important files/directories.

### Testing

State what was actually tested.

### Build

State whether the Windows build succeeded.

### Run Command

Provide the exact command to run the development application.

### Build Command

Provide the exact command to create the production Windows build.

### Known Limitations

Clearly identify anything that remains incomplete.

Do not claim functionality was tested if it was not actually tested.

---

# START

Begin by inspecting:

```text
F:\PS4 Downloader
```

Then create the implementation plan and proceed with the build.

The priority is:

**Reliability → Correctness → Download performance → Storage safety → UI polish.**
