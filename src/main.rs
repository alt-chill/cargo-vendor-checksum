use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Command, CommandFactory, Parser};
use clap_complete::{generate, Generator, Shell};
use clap_derive::{Args, Parser};
use serde::{Deserialize, Serialize};

#[derive(Args, Debug)]
#[group(required = true, multiple = false)]
struct Files {
    #[arg(long, alias = "files", short, value_name = "FILES")]
    files_in_vendor_dir: Vec<PathBuf>,

    #[arg(long, short, num_args(1..))]
    packages: Vec<OsString>,

    #[arg(long, short)]
    all: bool,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, infer_long_args(true))]
struct Cli {
    #[command(flatten)]
    files: Files,

    #[arg(long, default_value = "vendor", value_name = "DIR")]
    vendor: PathBuf,

    #[arg(long, required(false), value_name = "SHELL")]
    completion: Option<Shell>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Checksum {
    files: BTreeMap<PathBuf, String>,
    package: Option<String>,
    #[serde(skip)]
    path: PathBuf,
}

impl Checksum {
    pub fn new(cksum_file: &Path) -> Result<Self> {
        let cksum_str = &fs::read_to_string(cksum_file)
            .with_context(|| format!("failed to read checksum file `{}`", cksum_file.display()))?;
        let mut checksum: Checksum = serde_json::from_str(cksum_str).with_context(|| {
            format!("failed to parse checksum file `{}`", cksum_file.display())
        })?;

        checksum.path = cksum_file.to_owned();

        Ok(checksum)
    }

    pub fn write(self) -> Result<()> {
        let cksum_str = serde_json::to_string(&self)?;
        fs::write(&self.path, cksum_str)
            .with_context(|| format!("failed to write checksum file `{}`", self.path.display()))?;

        Ok(())
    }
}

fn get_packages(vendor: &Path) -> Result<Vec<OsString>> {
    Ok(fs::read_dir(vendor)
        .with_context(|| format!("failed to read vendor directory `{}`", vendor.display(),))?
        .filter_map(|r| r.map(|e| e.path().file_name().map(|n| n.to_owned())).ok().flatten())
        .collect::<Vec<_>>())
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
}

fn write_checksums(checksums: BTreeMap<OsString, Checksum>) -> Result<()> {
    for (_pkg, checksum) in checksums {
        checksum.write()?;
    }

    Ok(())
}

fn process_files_in_vendor_dir<V: AsRef<Path>>(
    vendor: V,
    files_in_vendor_dir: Vec<PathBuf>,
) -> Result<()> {
    let mut checksums: BTreeMap<OsString, Checksum> = BTreeMap::new();
    let vendor = vendor.as_ref();

    for file_in_vendor_dir in files_in_vendor_dir {
        let file_parts = file_in_vendor_dir.iter().collect::<Vec<&OsStr>>();
        if file_parts.len() < 2 {
            bail!(
                "File name should contain at least 2 parts but given `{}`",
                &file_in_vendor_dir.display()
            );
        }
        let pkg = file_parts[0].to_owned();
        let full_file = vendor.join(&file_in_vendor_dir);
        if !checksums.contains_key(&pkg) {
            let cksum_file = vendor.join(&pkg).join(".cargo-checksum.json");
            checksums.insert(pkg.to_owned(), Checksum::new(&cksum_file)?);
        }
        let file_in_pkg = file_parts[1..].iter().collect::<PathBuf>();
        let digest = sha256::try_digest(&full_file).with_context(|| {
            format!("failed to get checksum for file `{}`", full_file.display())
        })?;
        checksums
            .get_mut(&pkg)
            .expect("Checksum should be created")
            .files
            .insert(file_in_pkg, digest);
    }

    write_checksums(checksums)
}

fn process_packages<V: AsRef<Path>>(vendor: V, packages: Vec<OsString>) -> Result<()> {
    let mut checksums: BTreeMap<OsString, Checksum> = BTreeMap::new();
    let vendor = vendor.as_ref();

    for pkg in packages {
        let path = Path::new(&vendor).join(&pkg);
        if !checksums.contains_key(&pkg) {
            let cksum_file = path.join(".cargo-checksum.json");
            let mut checksum = Checksum::new(&cksum_file)?;
            for relative_file in checksum.files.to_owned().into_keys() {
                let file = path.join(&relative_file);
                let digest = sha256::try_digest(&file).with_context(|| {
                    format!("failed to get checksum for file `{}`", file.display())
                })?;
                checksum.files.insert(relative_file.to_owned(), digest);
            }
            checksums.insert(pkg.to_owned(), checksum);
        }
    }

    write_checksums(checksums)
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let vendor = args.vendor;
    let files_in_vendor_dir: Vec<PathBuf>;
    let packages: Vec<OsString>;

    if let Some(generator) = args.completion {
        print_completions(generator, &mut Cli::command());
    }

    if args.files.all {
        packages = get_packages(&vendor)?;
        files_in_vendor_dir = vec![];
    } else {
        files_in_vendor_dir = args.files.files_in_vendor_dir;
        packages = args.files.packages;
    }

    process_files_in_vendor_dir(&vendor, files_in_vendor_dir)?;
    process_packages(&vendor, packages)?;

    Ok(())
}
