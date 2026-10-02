//! Development-only getter measurements through real generated COM client/thunk code.
//! The same source records and iteration counts are used across the output migration.

use super::{neocogi_bench_begin, neocogi_bench_end};
use neocogi_com::{com_object, ComPtr, NcCopyToBufferOffset, NcStatus, NC_E_INVALID_ARG};
use neocogi_ui::*;
use std::{error::Error, hint::black_box, time::Instant};

#[com_object(IRenderStream)]
/// Provider owns native records; callers own all destination storage.
struct Source {
    /// Mixed alternatives exercise the same recursive wire codec as real drawing.
    records: Vec<RenderBatch>,
}

impl IRenderStream for Source {
    /// Prevents LTO from replacing the measured provider's work with a constant result.
    #[inline(never)]
    fn batches(&self, _: u64, mut output: NcCopyToBufferOffset<'_, RenderBatch>) -> Result<u32, NcStatus> {
        let first = usize::try_from(output.offset()).map_err(|_| neocogi_com::NC_E_OUT_OF_RANGE)?;
        let source = self.records.get(first..).ok_or(neocogi_com::NC_E_INVALID_ARG)?;
        let count = source.len().min(output.len());
        output.as_mut_slice()[..count].copy_from_slice(&source[..count]);
        Ok(count as u32)
    }

    /// The transport fixture contains records only; vertex access is unsupported.
    fn vertices(&self, _: u64, _: NcCopyToBufferOffset<'_, RenderVertex>) -> Result<u32, NcStatus> { Err(NC_E_INVALID_ARG) }
    /// No resource capabilities are needed to measure copied record transport.
    fn texture(&self, _: u64, _: u32) -> Result<ComPtr<ITextureAbi>, NcStatus> { Err(NC_E_INVALID_ARG) }
    /// Custom records are structural fixtures and have no renderer-specific object.
    fn item(&self, _: u64, _: u32) -> Result<ComPtr<IRenderItemAbi>, NcStatus> { Err(NC_E_INVALID_ARG) }
}

/// Makes every alternative observable without loading a renderer or GPU driver.
fn record(index: usize) -> RenderBatch {
    match index % 3 {
        0 => RenderBatch::Atlas { first: index as u32, count: 3 },
        1 => RenderBatch::Texture { first: index as u32 },
        _ => RenderBatch::Item {
            rect: Rect { x: index as i32, y: 2, width: 3, height: 4 },
            clip: Rect { x: 0, y: 0, width: 1000, height: 1000 },
        },
    }
}

/// Keeps the measured call opaque while still exercising the safe generated client.
#[inline(never)]
fn read(source: &ComPtr<IRenderStreamAbi>, destination: &mut [RenderBatch]) -> Result<(), NcStatus> {
    black_box(source).batches(1, NcCopyToBufferOffset::new(0, destination))?;
    black_box(destination);
    Ok(())
}

/// Reports cold setup and warmed per-call timings for small and large outputs.
pub(super) fn run(counts: bool) -> Result<(), Box<dyn Error>> {
    let iterations: u32 = match counts {
        true => 1,
        false => std::env::var("NEOCOGI_BENCH_ITERATIONS").ok().and_then(|value| value.parse().ok()).unwrap_or(3000),
    };
    for count in [1, 1024, 8192] {
        let source = Source::from_value(Source { records: (0..count).map(record).collect() }).query::<IRenderStreamAbi>()?;
        let mut destination = vec![RenderBatch::Atlas { first: 0, count: 0 }; count];
        let start = Instant::now();
        read(&source, &mut destination)?;
        println!("{{\"operation\":\"bulk_output_cold\",\"count\":{count},\"ns\":{}}}", start.elapsed().as_nanos());
        for _ in 0..4 { read(&source, &mut destination)?; }
        for sample in 0..if counts { 1 } else { 7 } {
            neocogi_bench_begin(5);
            let start = Instant::now();
            for _ in 0..iterations { read(&source, &mut destination)?; }
            let elapsed = start.elapsed();
            neocogi_bench_end();
            println!("{{\"operation\":\"bulk_output\",\"count\":{count},\"sample\":{sample},\"iterations\":{iterations},\"ns_per_iteration\":{}}}", elapsed.as_nanos() / u128::from(iterations));
        }
    }
    Ok(())
}
