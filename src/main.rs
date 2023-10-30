#![feature(array_chunks)]
#![feature(iter_array_chunks)]

// mod nacl;

use std::{
    env,
    sync::{atomic::AtomicUsize, Arc},
    time::{Duration, Instant},
};

use base64::{
    alphabet,
    engine::{GeneralPurpose, GeneralPurposeConfig},
};

static GENERATED: AtomicUsize = AtomicUsize::new(0);

const BASE64: GeneralPurpose =
    GeneralPurpose::new(&alphabet::STANDARD, GeneralPurposeConfig::new());

const KEY_LENGTH: usize = 32;
const KEY_B64_LENGTH: usize = 44;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

    let (tx, rx) = std::sync::mpsc::channel::<[[u8; KEY_LENGTH]; 2]>();

    let start = Instant::now();

    gpu();
}

use rand::{rngs::OsRng, Fill, RngCore};
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo},
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferInfo,
    },
    descriptor_set::{
        allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo},
        layout::{
            DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorSetLayoutCreateFlags,
            DescriptorSetLayoutCreateInfo, DescriptorType,
        },
        CopyDescriptorSet, PersistentDescriptorSet, WriteDescriptorSet,
    },
    device::{Device, DeviceCreateInfo, QueueCreateInfo, QueueFlags},
    instance::{Instance, InstanceCreateInfo},
    memory::{
        allocator::{
            AllocationCreateInfo, DeviceLayout, MemoryTypeFilter, StandardMemoryAllocator,
        },
        DeviceAlignment,
    },
    pipeline::{
        compute::ComputePipelineCreateInfo,
        layout::{PipelineDescriptorSetLayoutCreateInfo, PipelineLayoutCreateInfo},
        ComputePipeline, Pipeline, PipelineBindPoint, PipelineCreateFlags, PipelineLayout,
        PipelineShaderStageCreateInfo,
    },
    shader::{self, EntryPoint, ShaderModule, ShaderModuleCreateInfo},
    sync::{self, GpuFuture},
    VulkanLibrary,
};

fn gpu() {
    let library = VulkanLibrary::new().unwrap();
    let instance = Instance::new(library, InstanceCreateInfo::default()).unwrap();

    let physical_devices = instance
        .enumerate_physical_devices()
        .expect("could not enumerate devices")
        .collect::<Vec<_>>();

    eprintln!("Found {} devices", physical_devices.len());

    let physical_device = physical_devices
        .into_iter()
        .next()
        .expect("no devices available");

    let queue_family_index = physical_device
        .queue_family_properties()
        .iter()
        .position(|queue_family_properties| {
            queue_family_properties
                .queue_flags
                .contains(QueueFlags::COMPUTE)
        })
        .expect("couldn't find a graphical queue family") as u32;

    let (device, mut queues) = Device::new(
        physical_device,
        DeviceCreateInfo {
            // here we pass the desired queue family to use by index
            queue_create_infos: vec![QueueCreateInfo {
                queue_family_index,
                ..Default::default()
            }],
            ..Default::default()
        },
    )
    .expect("failed to create device");

    let queue = queues.next().unwrap();

    let memory_allocator = Arc::new(StandardMemoryAllocator::new_default(device.clone()));

    const PARALLELISM: u32 = 64;
    const LOCAL_SIZE: u32 = 16;

    let input_buffer = Buffer::from_iter(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::STORAGE_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        (0..(KEY_LENGTH as u32 * PARALLELISM))
            .map(|_| 0u32)
            .collect::<Vec<_>>(),
    )
    .expect("failed to create buffer");

    let output_buffer = Buffer::from_iter(
        memory_allocator.clone(),
        BufferCreateInfo {
            usage: BufferUsage::STORAGE_BUFFER,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ..Default::default()
        },
        (0..(KEY_LENGTH as u32 * PARALLELISM))
            .map(|_| 0u32)
            .collect::<Vec<_>>(),
    )
    .expect("failed to create buffer");

    let shader_u8 = include_bytes!("./shader.comp.spv");
    let mut shader_u32 = Vec::with_capacity((shader_u8.len() + 3) / 4);
    for chunk in shader_u8.array_chunks() {
        shader_u32.push(u32::from_le_bytes(*chunk));
    }

    let shader = ShaderModuleCreateInfo::new(&shader_u32);
    let shader = unsafe { ShaderModule::new(device.clone(), shader) }.unwrap();

    let descriptor_set_allocator = StandardDescriptorSetAllocator::new(
        device.clone(),
        StandardDescriptorSetAllocatorCreateInfo::default(),
    );

    // let descriptor_set_layout = DescriptorSetLayout::new(
    //     device.clone(),
    //     DescriptorSetLayoutCreateInfo {
    //         bindings: vec![(
    //             0,
    //             DescriptorSetLayoutBinding::descriptor_type(DescriptorType::StorageBuffer),
    //         )]
    //         .into_iter()
    //         .collect(),
    //         ..Default::default()
    //     },
    // )
    // .unwrap();

    // let stage = PipelineShaderStageCreateInfo::new(shader.entry_point("main").unwrap());
    // let layout = PipelineLayout::new(
    //     device.clone(),
    //     PipelineLayoutCreateInfo {
    //         // set_layouts: vec![descriptor_set_layout.clone()],
    //         ..Default::default()
    //     },
    // )
    // .unwrap();

    // let compute_pipeline = ComputePipeline::new(
    //     device.clone(),
    //     None,
    //     ComputePipelineCreateInfo::stage_layout(stage, layout),
    // )
    // .expect("failed to create compute pipeline");

    // let layout = compute_pipeline.layout().set_layouts().get(0).unwrap();

    // let descriptor_set = PersistentDescriptorSet::new(
    //     &descriptor_set_allocator,
    //     layout.clone(),
    //     [WriteDescriptorSet::buffer(0, input_buffer.clone())],
    //     [],
    // )
    // .unwrap();

    let pipeline = {
        let stage = PipelineShaderStageCreateInfo::new(shader.entry_point("main").unwrap());
        let layout = PipelineLayout::new(
            device.clone(),
            PipelineDescriptorSetLayoutCreateInfo::from_stages([&stage])
                .into_pipeline_layout_create_info(device.clone())
                .unwrap(),
        )
        .unwrap();

        ComputePipeline::new(
            device.clone(),
            None,
            ComputePipelineCreateInfo::stage_layout(stage, layout),
        )
        .unwrap()
    };

    let layout = pipeline.layout().set_layouts().get(0).unwrap();
    let set = PersistentDescriptorSet::new(
        &descriptor_set_allocator,
        layout.clone(),
        [
            WriteDescriptorSet::buffer(0, input_buffer.clone()),
            WriteDescriptorSet::buffer(1, output_buffer.clone()),
        ],
        [],
    )
    .unwrap();

    let command_buffer_allocator = StandardCommandBufferAllocator::new(
        device.clone(),
        StandardCommandBufferAllocatorCreateInfo::default(),
    );

    let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
        &command_buffer_allocator,
        queue.queue_family_index(),
        CommandBufferUsage::MultipleSubmit,
    )
    .unwrap();

    let work_group_counts = [PARALLELISM / LOCAL_SIZE, 1, 1];

    command_buffer_builder
        .bind_pipeline_compute(pipeline.clone())
        .unwrap()
        .bind_descriptor_sets(
            PipelineBindPoint::Compute,
            pipeline.layout().clone(),
            0,
            set,
        )
        .unwrap()
        .dispatch(work_group_counts)
        .unwrap();

    let command_buffer = command_buffer_builder.build().unwrap();

    for _ in 0..5 {
        let start = Instant::now();

        {
            let mut buf = input_buffer.write().unwrap();
            buf.iter_mut().for_each(|x| *x = OsRng.next_u32() & 0xff);

            // (0..(buf.len() / 8)).for_each(|idx| {
            //     buf[idx * 8] &= 0xf8_ff_ff_ff;
            //     buf[idx * 8 + 7] &= 0x7f_ff_ff_ff;
            //     buf[idx * 8 + 7] |= 0x40_00_00_00;
            // });
        };

        // {
        //     let sk = &input_buffer.read().unwrap()[0..32]
        //         .iter()
        //         .map(|&x| x as u8)
        //         .collect::<Vec<_>>();

        //     let sk64 = BASE64.encode(&sk);

        //     println!();
        //     println!("sk: {sk64} {sk:02x?}");
        // }

        let future = sync::now(device.clone())
            .then_execute(queue.clone(), command_buffer.clone())
            .unwrap()
            .then_signal_fence_and_flush()
            .unwrap();

        future.wait(None).unwrap();

        let keys = output_buffer
            .read()
            .unwrap()
            .iter()
            .map(|&x| x as u8)
            .array_chunks::<KEY_LENGTH>()
            .collect::<Vec<_>>();

        println!("{}ms for {} keys", start.elapsed().as_millis(), keys.len());
        println!(
            "{} keys/sec",
            keys.len() as f64 / start.elapsed().as_secs_f64()
        );

        // {
        //     let pk = &output_buffer.read().unwrap()[0..32]
        //         .iter()
        //         .map(|&x| x as u8)
        //         .collect::<Vec<_>>();

        //     let pk64 = BASE64.encode(&pk);

        //     println!("pk: {pk64} {pk:02x?}");
        // }

        // {
        //     let mut nc = [0u8; 32];
        //     curve25519_base(&mut nc, sk);
        //     let nc64 = BASE64.encode(&nc);

        //     println!("nc: {nc64} {nc:02x?}");
        // }

        // {
        //     let mut wg = std::process::Command::new("wg")
        //         .arg("pubkey")
        //         .stdin(std::process::Stdio::piped())
        //         .stdout(std::process::Stdio::piped())
        //         .spawn()
        //         .unwrap();

        //     wg.stdin.take().unwrap().write_all(sk64.as_bytes()).unwrap();

        //     let wg64 = String::from_utf8(wg.wait_with_output().unwrap().stdout).unwrap();
        //     let wg64 = wg64.trim_end();
        //     let wg = BASE64.decode(&wg64).unwrap();

        //     println!("wg: {wg64} {wg:02x?}");
        // }

        std::thread::sleep(Duration::from_millis(100));
    }
}
