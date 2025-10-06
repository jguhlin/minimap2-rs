use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use minimap2_sys::*;
use std::ffi::c_void;
use std::os::raw::{c_int, c_long};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// Benchmark data structures
struct WorkerData {
    counter: Arc<AtomicU64>,
    work_per_iteration: u64,
}

// Simple worker function for kt_for benchmarking
unsafe extern "C" fn kt_for_worker(data: *mut c_void, i: c_long, _tid: c_int) {
    let worker_data = unsafe { &*(data as *const WorkerData) };
    
    // Simulate some work - mathematical operations
    let mut sum = 0u64;
    for j in 0..worker_data.work_per_iteration {
        sum = sum.wrapping_add(i as u64).wrapping_mul(j + 1);
    }
    
    // Store result to prevent optimization
    worker_data.counter.fetch_add(sum, Ordering::Relaxed);
}

// Pipeline worker function for kt_pipeline benchmarking
unsafe extern "C" fn kt_pipeline_worker(
    shared: *mut c_void, 
    step: c_int, 
    data: *mut c_void
) -> *mut c_void {
    let worker_data = unsafe { &*(shared as *const WorkerData) };
    
    match step {
        0 => {
            // First step - generate data
            let value = 42u64 * (step + 1) as u64;
            Box::into_raw(Box::new(value)) as *mut c_void
        }
        1 => {
            // Middle step - process data
            if !data.is_null() {
                let input = unsafe { Box::from_raw(data as *mut u64) };
                let processed = *input * 2 + worker_data.work_per_iteration;
                Box::into_raw(Box::new(processed)) as *mut c_void
            } else {
                std::ptr::null_mut()
            }
        }
        2 => {
            // Final step - consume data
            if !data.is_null() {
                let input = unsafe { Box::from_raw(data as *mut u64) };
                worker_data.counter.fetch_add(*input, Ordering::Relaxed);
            }
            std::ptr::null_mut()
        }
        _ => std::ptr::null_mut(),
    }
}

fn benchmark_kt_for(c: &mut Criterion) {
    let mut group = c.benchmark_group("kt_for");
    
    let work_sizes = [100, 1000, 10000];
    let thread_counts = [1, 2, 4, 8];
    
    for work_size in work_sizes.iter() {
        for thread_count in thread_counts.iter() {
            let worker_data = WorkerData {
                counter: Arc::new(AtomicU64::new(0)),
                work_per_iteration: 1000, // Fixed computational work per iteration
            };
            
            group.throughput(Throughput::Elements(*work_size as u64));
            group.bench_with_input(
                BenchmarkId::from_parameter(format!("{}items_{}threads", work_size, thread_count)),
                work_size,
                |b, &work_size| {
                    b.iter(|| {
                        worker_data.counter.store(0, Ordering::Relaxed);
                        unsafe {
                            kt_for(
                                *thread_count,
                                Some(kt_for_worker),
                                &worker_data as *const _ as *mut c_void,
                                work_size as c_long,
                            );
                        }
                        // Return the counter value to prevent dead code elimination
                        worker_data.counter.load(Ordering::Relaxed)
                    });
                },
            );
        }
    }
    group.finish();
}

fn benchmark_kt_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("kt_pipeline");
    
    let thread_counts = [1, 2, 3, 4];
    
    for thread_count in thread_counts.iter() {
        let worker_data = WorkerData {
            counter: Arc::new(AtomicU64::new(0)),
            work_per_iteration: 1000,
        };
        
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", thread_count)),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    worker_data.counter.store(0, Ordering::Relaxed);
                    unsafe {
                        kt_pipeline(
                            thread_count,
                            Some(kt_pipeline_worker),
                            &worker_data as *const _ as *mut c_void,
                            3, // 3 steps in pipeline
                        );
                    }
                    // Return the counter value to prevent dead code elimination
                    worker_data.counter.load(Ordering::Relaxed)
                });
            },
        );
    }
    group.finish();
}

// Benchmark specifically designed to test threading overhead
fn benchmark_threading_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("threading_overhead");
    
    // Very light work to emphasize threading overhead
    let light_worker_data = WorkerData {
        counter: Arc::new(AtomicU64::new(0)),
        work_per_iteration: 1, // Minimal work
    };
    
    let thread_counts = [1, 2, 4, 8, 16];
    let work_size = 10000; // Many small tasks
    
    for thread_count in thread_counts.iter() {
        group.throughput(Throughput::Elements(work_size));
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}threads", thread_count)),
            thread_count,
            |b, &thread_count| {
                b.iter(|| {
                    light_worker_data.counter.store(0, Ordering::Relaxed);
                    unsafe {
                        kt_for(
                            thread_count,
                            Some(kt_for_worker),
                            &light_worker_data as *const _ as *mut c_void,
                            work_size as c_long,
                        );
                    }
                    light_worker_data.counter.load(Ordering::Relaxed)
                });
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    benchmark_kt_for,
    benchmark_kt_pipeline,
    benchmark_threading_overhead
);
criterion_main!(benches);