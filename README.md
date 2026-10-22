# Lightman

**Lightman** adalah HTTP client desktop yang ringan dan mengutamakan privasi. Dibangun dengan Rust sebagai backend (melalui Tauri 2) dan React 19 + TypeScript sebagai frontend, Lightman memungkinkan Anda mengirim dan menguji permintaan HTTP, mengelola koleksi request, environment variables, serta menyimpan riwayat respons — semuanya secara lokal tanpa mengirim data ke server pihak ketiga.

---

## Daftar Isi

- [Prasyarat](#prasyarat)
- [Instalasi Dependensi](#instalasi-dependensi)
- [Menjalankan Development](#menjalankan-development)
- [Build Produksi](#build-produksi)
- [Deployment / Distribusi](#deployment--distribusi)
- [Menjalankan Tes](#menjalankan-tes)
- [IDE yang Direkomendasikan](#ide-yang-direkomendasikan)
- [Struktur Proyek](#struktur-proyek)

---

## Prasyarat

Pastikan semua tools berikut sudah terinstal sebelum memulai:

### 1. Node.js & npm
- Node.js **v18+** — [Download](https://nodejs.org/)
- Verifikasi: `node -v` dan `npm -v`

### 2. Rust & Cargo
- Rust **stable** terbaru via [rustup](https://rustup.rs/)
- Verifikasi: `rustc --version` dan `cargo --version`

### 3. Dependensi Sistem per Platform

#### Windows
- **Microsoft Visual Studio C++ Build Tools** — [Download](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  - Centang: *Desktop development with C++*
- **WebView2** — biasanya sudah terinstal di Windows 10/11. Jika belum: [Download](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

#### Linux
Pastikan paket berikut terinstal (nama paket bisa berbeda per distro):

| Distro | Perintah |
|--------|----------|
| Ubuntu / Debian | `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev` |
| Fedora | `sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel gcc` |
| Arch Linux | `sudo pacman -S webkit2gtk-4.1 base-devel curl wget file openssl appindicator-gtk3 librsvg` |

> **Catatan:** Tauri 2 membutuhkan `webkit2gtk-4.1` (bukan `4.0`). Jika distro Anda masih menggunakan `4.0`, lihat dokumentasi [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/).

#### macOS
- **Xcode Command Line Tools** — jalankan: `xcode-select --install`
- Tidak ada dependensi tambahan, macOS sudah menyertakan WebKit bawaan.

Referensi lengkap: [Tauri Prerequisites](https://v2.tauri.app/start/prerequisites/)

---

## Instalasi Dependensi

Clone repo dan install dependensi frontend:

```bash
git clone <repo-url>
cd lightman
npm install
```

Dependensi Rust (crate) akan diunduh otomatis saat pertama kali build atau menjalankan dev.

---

## Menjalankan Development

Jalankan aplikasi dalam mode development dengan hot-reload:

```bash
npm run tauri dev
```

Perintah ini akan:
1. Menjalankan Vite dev server di `http://localhost:1420`
2. Mengkompilasi backend Rust
3. Membuka jendela aplikasi desktop Lightman

Setiap perubahan pada file TypeScript/React akan langsung ter-refresh. Perubahan pada kode Rust akan memicu recompile otomatis.

---

## Build Produksi

### Build Frontend Saja

Untuk build frontend (TypeScript + React) tanpa menghasilkan binary:

```bash
npm run build
```

Output akan ada di folder `dist/`.

### Build Aplikasi Desktop (Tauri)

Perintah build untuk semua platform sama, namun harus dijalankan di OS masing-masing:

```bash
npm run tauri build
```

Proses ini akan:
1. Build frontend via `npm run build`
2. Compile Rust backend dengan profil `release` (optimasi penuh: LTO, strip binary, dll.)
3. Menghasilkan installer di `src-tauri/target/release/bundle/`

> **Catatan:** Build pertama kali bisa memakan waktu 5–15 menit karena Rust mengkompilasi semua crate dari awal.

### Hasil Build per OS

#### Windows

```bash
npm run tauri build
```

| Format | Path | Keterangan |
|--------|------|------------|
| `.msi` | `src-tauri/target/release/bundle/msi/` | Installer MSI |
| `.exe` | `src-tauri/target/release/bundle/nsis/` | Installer NSIS |
| `.exe` | `src-tauri/target/release/lightman.exe` | Binary portable |

Contoh output:
```
src-tauri/target/release/bundle/msi/lightman_0.1.0_x64_en-US.msi
src-tauri/target/release/bundle/nsis/lightman_0.1.0_x64-setup.exe
src-tauri/target/release/lightman.exe
```

#### Linux

```bash
npm run tauri build
```

| Format | Path | Keterangan |
|--------|------|------------|
| `.deb` | `src-tauri/target/release/bundle/deb/` | Installer Debian/Ubuntu |
| `.rpm` | `src-tauri/target/release/bundle/rpm/` | Installer Fedora/RHEL |
| `.AppImage` | `src-tauri/target/release/bundle/appimage/` | Portable AppImage |

Contoh output:
```
src-tauri/target/release/bundle/deb/lightman_0.1.0_amd64.deb
src-tauri/target/release/bundle/rpm/lightman-0.1.0-1.x86_64.rpm
src-tauri/target/release/bundle/appimage/lightman_0.1.0_amd64.AppImage
```

#### macOS

```bash
npm run tauri build
```

| Format | Path | Keterangan |
|--------|------|------------|
| `.dmg` | `src-tauri/target/release/bundle/dmg/` | Installer DMG |
| `.app` | `src-tauri/target/release/bundle/macos/` | Bundel aplikasi macOS |

Contoh output:
```
src-tauri/target/release/bundle/dmg/Lightman_0.1.0_aarch64.dmg
src-tauri/target/release/bundle/macos/Lightman.app
```

> Build menghasilkan format installer sesuai OS tempat Anda melakukan build. Untuk membuat build lintas platform (cross-compile), gunakan CI/CD pipeline (mis. GitHub Actions) dengan runner masing-masing OS.

---

## Deployment / Distribusi

Lightman adalah aplikasi desktop, sehingga "deployment" berarti mendistribusikan binary atau installer kepada pengguna.

### Distribusi Manual

1. Jalankan `npm run tauri build` di OS target
2. Ambil file installer dari `src-tauri/target/release/bundle/`
3. Distribusikan ke pengguna:
   - **Windows:** `.msi` atau `.exe`
   - **Linux:** `.deb`, `.rpm`, atau `.AppImage`
   - **macOS:** `.dmg` atau `.app`

### Update Versi

Sebelum build rilis baru, update versi di dua tempat:

- `package.json` → field `"version"`
- `src-tauri/tauri.conf.json` → field `"version"`

### Konfigurasi Bundle

Konfigurasi build dan bundle ada di `src-tauri/tauri.conf.json`:

```json
{
  "productName": "Lightman",
  "version": "0.1.0",
  "identifier": "com.lightman.app",
  "bundle": {
    "active": true,
    "targets": "all"
  }
}
```

Ubah `"targets"` untuk memilih format spesifik, misalnya `["msi"]` untuk hanya menghasilkan installer MSI.

---

## Menjalankan Tes

### Tes Rust (Unit, Integrasi, Property)

Jalankan seluruh test suite Rust:

```bash
cd src-tauri
cargo test
```

Jalankan kategori tes tertentu:

```bash
# Unit tests saja
cargo test --test test_collection_service
cargo test --test test_http_executor
cargo test --test test_var_interpolator

# Integration tests
cargo test --test test_send_request_e2e
cargo test --test test_export_import_file

# Property-based tests
cargo test --test prop_auth
cargo test --test prop_variable_resolution
```

Jalankan semua tes dengan output verbose:

```bash
cargo test -- --nocapture
```

---

## IDE yang Direkomendasikan

- [VS Code](https://code.visualstudio.com/) dengan ekstensi:
  - [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
  - [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
  - [ESLint](https://marketplace.visualstudio.com/items?itemName=dbaeumer.vscode-eslint)

---

## Struktur Proyek

```
lightman/
├── src/                        # Frontend (React + TypeScript)
│   ├── api/                    # Layer komunikasi dengan Tauri commands
│   ├── components/             # Komponen UI React
│   ├── stores/                 # State management (Zustand)
│   └── types/                  # TypeScript type definitions
├── src-tauri/                  # Backend (Rust + Tauri)
│   ├── src/
│   │   ├── commands/           # Tauri command handlers
│   │   ├── services/           # Business logic
│   │   ├── models/             # Data models
│   │   └── storage/            # Persistensi data (SQLite)
│   ├── tests/
│   │   ├── unit/               # Unit tests
│   │   ├── integration/        # Integration tests
│   │   └── property/           # Property-based tests
│   ├── Cargo.toml
│   └── tauri.conf.json
├── package.json
└── index.html
```
