use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const FONT_URL: &str =
    "https://raw.githubusercontent.com/ryanoasis/nerd-fonts/v3.3.0/patched-fonts/JetBrainsMono/Ligatures/Regular/JetBrainsMonoNerdFontMono-Regular.ttf";
const FONT_NAME: &str = "JetBrainsMonoNerdFontMono-Regular.ttf";

const HELIX_VERSION: &str = "25.07.1";
const HELIX_ARCHIVE_NAME: &str = "helix-25.07.1-x86_64-windows.zip";
const HELIX_URL: &str =
    "https://github.com/helix-editor/helix/releases/download/25.07.1/helix-25.07.1-x86_64-windows.zip";

const SPELL_DICT_REV: &str = "8cfea406b505e4d7df52d5a19bce525df98c54ab";
const SPELL_AFF_URL: &str =
    "https://raw.githubusercontent.com/wooorm/dictionaries/8cfea406b505e4d7df52d5a19bce525df98c54ab/dictionaries/es-CL/index.aff";
const SPELL_DIC_URL: &str =
    "https://raw.githubusercontent.com/wooorm/dictionaries/8cfea406b505e4d7df52d5a19bce525df98c54ab/dictionaries/es-CL/index.dic";
const SPELL_LICENSE_URL: &str =
    "https://raw.githubusercontent.com/wooorm/dictionaries/8cfea406b505e4d7df52d5a19bce525df98c54ab/dictionaries/es-CL/license";

fn main() {
    println!("cargo:rerun-if-env-changed=HELIX_SST_FONT_FILE");
    println!("cargo:rerun-if-env-changed=HELIX_SST_ARCHIVE");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR no definido"));
    ensure_font(&out_dir);
    ensure_spell_dictionary(&out_dir);

    #[cfg(windows)]
    ensure_helix_archive(&out_dir);
}

fn curl_program() -> PathBuf {
    #[cfg(windows)]
    {
        let root = env::var_os("SystemRoot")
            .or_else(|| env::var_os("WINDIR"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let path = root.join("System32").join("curl.exe");
        if !path.is_file() {
            panic!("No se encontró curl.exe del sistema en {}", path.display());
        }
        path
    }

    #[cfg(not(windows))]
    {
        PathBuf::from("curl")
    }
}

fn download(url: &str, destination: &Path, description: &str) {
    let status = Command::new(curl_program())
        .args(["-L", "--fail", "--silent", "--show-error", url, "-o"])
        .arg(destination)
        .status()
        .unwrap_or_else(|error| panic!("No se pudo iniciar curl para {description}: {error}"));
    if !status.success() {
        panic!("No se pudo descargar {description}");
    }
}

fn ensure_font(out_dir: &Path) {
    let destination = out_dir.join(FONT_NAME);
    if let Some(source) = env::var_os("HELIX_SST_FONT_FILE") {
        fs::copy(Path::new(&source), &destination)
            .expect("No se pudo copiar HELIX_SST_FONT_FILE");
    } else if !destination.is_file() {
        download(FONT_URL, &destination, "JetBrainsMono Nerd Font");
    }
    if fs::metadata(&destination).map(|m| m.len()).unwrap_or(0) < 100_000 {
        panic!("La Nerd Font embebida parece incompleta");
    }
}

fn ensure_spell_dictionary(out_dir: &Path) {
    for (name, url, minimum) in [
        ("helix-sst-es-CL.aff", SPELL_AFF_URL, 50_000u64),
        ("helix-sst-es-CL.dic", SPELL_DIC_URL, 200_000u64),
        ("helix-sst-es-CL.LICENSE", SPELL_LICENSE_URL, 500u64),
    ] {
        let destination = out_dir.join(name);
        if !destination.is_file() {
            download(
                url,
                &destination,
                &format!("diccionario ortográfico es-CL de Helix-SST ({SPELL_DICT_REV})"),
            );
        }
        if fs::metadata(&destination).map(|m| m.len()).unwrap_or(0) < minimum {
            panic!("El archivo de diccionario {name} parece incompleto");
        }
    }
}

#[cfg(windows)]
fn ensure_helix_archive(out_dir: &Path) {
    let destination = out_dir.join(HELIX_ARCHIVE_NAME);
    if let Some(source) = env::var_os("HELIX_SST_ARCHIVE") {
        fs::copy(Path::new(&source), &destination)
            .expect("No se pudo copiar HELIX_SST_ARCHIVE");
    } else if !destination.is_file() {
        download(
            HELIX_URL,
            &destination,
            &format!("Helix {HELIX_VERSION}"),
        );
    }
    if fs::metadata(&destination).map(|m| m.len()).unwrap_or(0) < 1_000_000 {
        panic!("El paquete de Helix embebido parece incompleto");
    }
}
