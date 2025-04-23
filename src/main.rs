mod cli;
mod device;

use anyhow::{Context, Result};
use clap::Parser;
use cli::Cli;
use tracing::{info, trace, Level};
use tracing_subscriber::FmtSubscriber;
use vfio_bindings::bindings::vfio::{
    vfio_region_info, VFIO_PCI_CONFIG_REGION_INDEX, VFIO_PCI_NUM_IRQS, VFIO_PCI_NUM_REGIONS,
    VFIO_REGION_INFO_FLAG_READ, VFIO_REGION_INFO_FLAG_WRITE,
};
use vfio_user::{IrqInfo, Server, ServerBackend};

use device::{
    bus::{BusDevice, Request, RequestSize, SingleThreadedBusDevice},
    pci::config_space::{ConfigSpace, ConfigSpaceBuilder},
};

#[derive(Debug)]
struct PciDevice {
    config_space: ConfigSpace,
}

impl PciDevice {
    fn new() -> Self {
        Self {
            // 00:14.0 USB controller [0c03]: Intel Corporation Alder Lake-S PCH USB 3.2 Gen 2x2 XHCI Controller [8086:7ae0] (rev 11)
            config_space: ConfigSpaceBuilder::new(0x8086, 0x7ae0)
                // TODO check
                .class(0x0c, 0x03, 0x30)
                // TODO Should be a 64-bit BAR.
                .mem32_nonprefetchable_bar(0, 4 * 0x1000)
                .config_space(),
        }
    }
}

impl ServerBackend for PciDevice {
    fn region_read(
        &mut self,
        region: u32,
        offset: u64,
        data: &mut [u8],
    ) -> std::result::Result<(), std::io::Error> {
        trace!("read  region {region} offset {offset:#x}+{}", data.len());
        let value: u64 = match region {
            VFIO_PCI_CONFIG_REGION_INDEX => self.config_space.read(Request::new(
                offset,
                RequestSize::try_from(data.len() as u64).unwrap(),
            )),

            _ => !0u64,
        };

        data.copy_from_slice(&value.to_le_bytes()[0..data.len()]);

        Ok(())
    }

    fn region_write(
        &mut self,
        region: u32,
        offset: u64,
        data: &[u8],
    ) -> std::result::Result<(), std::io::Error> {
        trace!("write region {region} offset {offset:#x}+{}", data.len());
        match region {
            VFIO_PCI_CONFIG_REGION_INDEX => self.config_space.write(
                Request::new(offset, RequestSize::try_from(data.len() as u64).unwrap()),
                match data.len() {
                    1 => data[0].into(),
                    2 => {
                        let val: [u8; 2] = data.try_into().unwrap();
                        u16::from_le_bytes(val).into()
                    }

                    4 => {
                        let val: [u8; 4] = data.try_into().unwrap();
                        u32::from_le_bytes(val).into()
                    }
                    _ => todo!(),
                },
            ),

            _ => todo!(),
        }

        Ok(())
    }

    fn dma_map(
        &mut self,
        flags: vfio_user::DmaMapFlags,
        offset: u64,
        address: u64,
        size: u64,
        fd: Option<&std::fs::File>,
    ) -> std::result::Result<(), std::io::Error> {
        info!("dma_map flags = {flags:?} offset = {offset} address = {address} size = {size} fd = {fd:?}");
        Ok(())
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

fn create_irqs() -> Vec<IrqInfo> {
    let mut irqs = Vec::with_capacity(VFIO_PCI_NUM_IRQS as usize);
    for index in 0..VFIO_PCI_NUM_IRQS {
        let irq = IrqInfo {
            index,
            count: 0,
            flags: 0,
        };

        irqs.push(irq);
    }

    irqs
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

    let mut backend = PciDevice::new();
    let s = Server::new(&args.socket, true, create_irqs(), regions)
        .context("Failed to create vfio-user server")?;

    s.run(&mut backend)
        .context("Failed to start vfio-user server")?;
    Ok(())
}
