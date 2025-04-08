mod cli;
mod device;

use anyhow::{Context, Result};
use clap::Parser;
use cli::Cli;
use tracing::{info, trace, Level};
use tracing_subscriber::FmtSubscriber;
use vfio_bindings::bindings::vfio::{
    vfio_region_info, VFIO_PCI_CONFIG_REGION_INDEX, VFIO_PCI_NUM_REGIONS,
    VFIO_REGION_INFO_FLAG_READ, VFIO_REGION_INFO_FLAG_WRITE,
};
use vfio_user::{Server, ServerBackend};

#[derive(Debug, Default)]
struct Backend {}

impl Backend {
    fn new() -> Self {
        Default::default()
    }
}

impl ServerBackend for Backend {
    fn region_read(
        &mut self,
        region: u32,
        offset: u64,
        data: &mut [u8],
    ) -> std::result::Result<(), std::io::Error> {
        trace!("region {region} offset {offset:#x}+{}", data.len());
        data.fill(0xff);
        Ok(())
    }

    fn region_write(
        &mut self,
        _region: u32,
        _offset: u64,
        _data: &[u8],
    ) -> std::result::Result<(), std::io::Error> {
        todo!()
    }

    fn dma_map(
        &mut self,
        _flags: vfio_user::DmaMapFlags,
        _offset: u64,
        _address: u64,
        _size: u64,
        _fd: Option<&std::fs::File>,
    ) -> std::result::Result<(), std::io::Error> {
        todo!()
    }

    fn dma_unmap(
        &mut self,
        _flags: vfio_user::DmaUnmapFlags,
        _address: u64,
        _size: u64,
    ) -> std::result::Result<(), std::io::Error> {
        todo!()
    }

    fn reset(&mut self) -> std::result::Result<(), std::io::Error> {
        todo!()
    }

    fn set_irqs(
        &mut self,
        _index: u32,
        _flags: u32,
        _start: u32,
        _count: u32,
        _fds: Vec<std::fs::File>,
    ) -> std::result::Result<(), std::io::Error> {
        todo!()
    }
}

fn main() -> Result<()> {
    let args = Cli::parse();

    let subscriber = FmtSubscriber::builder()
        .with_max_level(match args.verbose {
            0 => Level::INFO,
            1 => Level::DEBUG,
            _ => Level::TRACE,
        })
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set global tracing subscriber")?;

    // Log messages from the log crate as well.
    tracing_log::LogTracer::init()?;

    info!("We're up!");

    let regions: Vec<vfio_region_info> = (0..VFIO_PCI_NUM_REGIONS)
        .map(|i| match i {
            VFIO_PCI_CONFIG_REGION_INDEX => vfio_region_info {
                argsz: size_of::<vfio_region_info>() as u32,
                index: i,
                size: 256,
                flags: VFIO_REGION_INFO_FLAG_READ | VFIO_REGION_INFO_FLAG_WRITE,
                ..Default::default()
            },

            _ => vfio_region_info {
                argsz: size_of::<vfio_region_info>() as u32,
                index: i,
                ..Default::default()
            },
        })
        .collect();

    let mut backend = Backend::new();
    let s = Server::new(&args.socket, true, vec![], regions)
        .context("Failed to create vfio-user server")?;
    s.run(&mut backend)
        .context("Failed to start vfio-user server")?;
    Ok(())
}
