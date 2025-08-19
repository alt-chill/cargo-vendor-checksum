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
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;
use serde::{Deserialize, Serialize};

#[derive(Args, Debug)]
#[group(required = true, multiple = false)]
struct Files {
    /// Update checksum for specified vendored files
    #[arg(long, alias = "files", short, num_args(1..),  value_name = "FILES")]
    files_in_vendor_dir: Vec<PathBuf>,
    /// Run batch process for specified vendored packages
    #[arg(long, short, num_args(1..))]
    packages: Vec<OsString>,
    /// Run batch process for all vendor packages
    #[arg(long, short)]
    all: bool,

    #[arg(long, required(false), value_name = "SHELL")]
    completion: Option<Shell>,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None, infer_long_args(true))]
struct Cli {
    #[command(flatten)]
    files: Files,
    /// Specify the path of vendor folder, when running not from repository directory
    #[arg(long, default_value = "vendor", value_name = "DIR")]
    vendor: PathBuf,
    /// Set 'true' to remove checksum for missing files
    #[arg(long)]
    ignore_missing: bool,
    /// Limit the number of threads or this number will be set automatically
    #[arg(long, value_name("NUM"))]
    num_threads: Option<usize>,
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
    files_in_vendor_dir: &[PathBuf],
    ignore_missing: bool,
) -> Result<()> {
    let vendor = vendor.as_ref();

    let results = files_in_vendor_dir.par_iter().map(|file_in_vendor_dir| {
        let file_parts = file_in_vendor_dir.iter().collect::<Vec<&OsStr>>();
        if file_parts.len() < 2 {
            bail!(
                "File name should contain at least 2 parts but given `{}`",
                &file_in_vendor_dir.display()
            );
        }
        let pkg = file_parts[0].to_owned();
        let full_file = vendor.join(file_in_vendor_dir);
        let file_in_pkg = file_parts[1..].iter().collect::<PathBuf>();

        if ignore_missing && !full_file.exists() {
            return Ok((pkg, file_in_pkg, None));
        }

        let digest = sha256::try_digest(&full_file).with_context(|| {
            format!("failed to get checksum for file `{}`", full_file.display())
        })?;
        Ok((pkg, file_in_pkg, Some(digest)))
    });

    let mut checksums: BTreeMap<OsString, Checksum> = BTreeMap::new();
    for result in results.collect::<Vec<_>>() {
        let (pkg, file_in_pkg, digest) = result?;
        if !checksums.contains_key(&pkg) {
            let cksum_file = vendor.join(&pkg).join(".cargo-checksum.json");
            checksums.insert(pkg.to_owned(), Checksum::new(&cksum_file)?);
        }
        let files = &mut checksums.get_mut(&pkg).expect("Checksum should be created").files;
        if let Some(digest) = digest {
            files.insert(file_in_pkg, digest);
        } else {
            files.remove(&file_in_pkg);
        }
    }

    write_checksums(checksums)
}

fn process_packages<V: AsRef<Path>>(
    vendor: V,
    packages: &[OsString],
    ignore_missing: bool,
) -> Result<()> {
    let vendor = vendor.as_ref();

    packages.par_iter().try_for_each(|pkg| -> Result<()> {
        let path = Path::new(&vendor).join(pkg);
        let cksum_file = path.join(".cargo-checksum.json");
        let mut checksum = Checksum::new(&cksum_file)?;
        let results = checksum
            .files
            .to_owned()
            .into_keys()
            .collect::<Vec<_>>()
            .par_iter()
            .map(|relative_file| -> Result<_> {
                let file = path.join(relative_file);
                if ignore_missing && !file.exists() {
                    return Ok((relative_file.to_owned(), None));
                }
                let digest = sha256::try_digest(&file).with_context(|| {
                    format!("failed to get checksum for file `{}`", file.display())
                })?;
                Ok((relative_file.to_owned(), Some(digest)))
            })
            .collect::<Vec<_>>();

        for result in results {
            let (file, digest) = result?;
            if let Some(digest) = digest {
                checksum.files.insert(file, digest);
            } else {
                checksum.files.remove(&file);
            }
        }

        checksum.write()
    })
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let vendor = args.vendor;

    if let Some(generator) = args.files.completion {
        print_completions(generator, &mut Cli::command());
    }

    let mut thread_pool_builder = ThreadPoolBuilder::new();
    if let Some(num_threads) = args.num_threads {
        thread_pool_builder = thread_pool_builder.num_threads(num_threads);
    }
    let thread_pool = thread_pool_builder.build()?;

    thread_pool.install(|| {
        if !args.files.files_in_vendor_dir.is_empty() {
            process_files_in_vendor_dir(
                &vendor,
                &args.files.files_in_vendor_dir,
                args.ignore_missing,
            )
        } else if args.files.all {
            process_packages(&vendor, &get_packages(&vendor)?, args.ignore_missing)
        } else {
            process_packages(&vendor, &args.files.packages, args.ignore_missing)
        }
    })?;

    Ok(())
}
