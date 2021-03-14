use bindings::{
    windows::win32::direct3d11::*, windows::win32::direct3d12::*, windows::win32::direct3d_hlsl::*,
    windows::win32::direct_composition::*, windows::win32::dxgi::*, windows::win32::gdi::*,
    windows::win32::menus_and_resources::*, windows::win32::system_services::*,
    windows::win32::windows_and_messaging::*,
};
use std::convert::TryInto;
use std::ptr::null_mut;
use windows::{Abi, Interface};

const NUM_OF_FRAMES: usize = 2;

#[allow(dead_code)]
struct Window {
    hwnd: HWND,
    factory: IDXGIFactory4,
    adapter: IDXGIAdapter1,
    device: ID3D12Device,
    queue: ID3D12CommandQueue,
    allocator: ID3D12CommandAllocator,
    comp_device: IDCompositionDevice,
    swap_chain: IDXGISwapChain3,
    comp_target: IDCompositionTarget,
    comp_visual: IDCompositionVisual,
    rtv_desc_heap: ID3D12DescriptorHeap,
    rtv_desc_size: usize,
    resources: [ID3D12Resource; NUM_OF_FRAMES],
    root_signature: ID3D12RootSignature,
    list_graphics_direct: ID3D12GraphicsCommandList,
    // resources: Vec<ID3D12Resource>,
    // pipeline_state: ID3D12PipelineState,
    // root_signature: ID3D12RootSignature,
}

impl Window {
    pub fn new(hwnd: HWND) -> windows::Result<Self> {
        // let debug = unsafe {
        //     let mut ptr: Option<ID3D12Debug> = None;
        //     D3D12GetDebugInterface(&ID3D12Debug::IID, ptr.set_abi()).and_some(ptr)
        // }
        // .expect("Unable to create debug layer");

        // unsafe {
        //     debug.EnableDebugLayer();
        // }

        let factory = unsafe {
            let mut ptr: Option<IDXGIFactory4> = None;
            CreateDXGIFactory2(0, &IDXGIFactory4::IID, ptr.set_abi()).and_some(ptr)
        }?;

        let adapter = (0..99)
            .into_iter()
            .find_map(|i| unsafe {
                let mut ptr: Option<IDXGIAdapter1> = None;
                factory.EnumAdapters1(i, &mut ptr).and_some(ptr).ok()
            })
            .expect("Could not find d3d adapter");

        let device = unsafe {
            let mut ptr: Option<ID3D12Device> = None;
            D3D12CreateDevice(
                &adapter, // None for default adapter
                D3D_FEATURE_LEVEL::D3D_FEATURE_LEVEL_11_0,
                &ID3D12Device::IID,
                ptr.set_abi(),
            )
            .and_some(ptr)
        }?;

        let queue = unsafe {
            let mut ptr: Option<ID3D12CommandQueue> = None;
            let desc = D3D12_COMMAND_QUEUE_DESC {
                r#type: D3D12_COMMAND_LIST_TYPE::D3D12_COMMAND_LIST_TYPE_DIRECT,
                priority: D3D12_COMMAND_QUEUE_PRIORITY::D3D12_COMMAND_QUEUE_PRIORITY_HIGH.0,
                flags: D3D12_COMMAND_QUEUE_FLAGS::D3D12_COMMAND_QUEUE_FLAG_NONE,
                node_mask: 0,
            };
            device
                .CreateCommandQueue(&desc, &ID3D12CommandQueue::IID, ptr.set_abi())
                .and_some(ptr)
        }?;

        let allocator = unsafe {
            let mut ptr: Option<ID3D12CommandAllocator> = None;
            device
                .CreateCommandAllocator(
                    D3D12_COMMAND_LIST_TYPE::D3D12_COMMAND_LIST_TYPE_DIRECT,
                    &ID3D12CommandAllocator::IID,
                    ptr.set_abi(),
                )
                .and_some(ptr)
        }?;

        // Composition device
        let comp_device = unsafe {
            let mut ptr: Option<IDCompositionDevice> = None;
            DCompositionCreateDevice(None, &IDCompositionDevice::IID, ptr.set_abi()).and_some(ptr)
        }?;

        // Create swap chain for composition
        let swap_chain = unsafe {
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                alpha_mode: DXGI_ALPHA_MODE::DXGI_ALPHA_MODE_PREMULTIPLIED,
                buffer_count: NUM_OF_FRAMES as _,
                width: 1024,
                height: 1024,
                format: DXGI_FORMAT::DXGI_FORMAT_B8G8R8A8_UNORM,
                flags: 0,
                buffer_usage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                sample_desc: DXGI_SAMPLE_DESC {
                    count: 1,
                    quality: 0,
                },
                scaling: DXGI_SCALING::DXGI_SCALING_STRETCH,
                stereo: BOOL(1),
                swap_effect: DXGI_SWAP_EFFECT::DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            };
            let mut ptr: Option<IDXGISwapChain1> = None;
            factory
                .CreateSwapChainForComposition(&queue, &desc, None, &mut ptr)
                .and_some(ptr)
        }?
        .cast::<IDXGISwapChain3>()?;

        // Create IDCompositionTarget for the window
        let comp_target = unsafe {
            let mut ptr = None;
            comp_device
                .CreateTargetForHwnd(hwnd, BOOL(1), &mut ptr)
                .and_some(ptr)
        }?;

        // Create IDCompositionVisual for the window
        let comp_visual = unsafe {
            let mut ptr = None;
            comp_device.CreateVisual(&mut ptr).and_some(ptr)
        }?;

        // Set swap_chain and the root visual and commit
        unsafe {
            comp_visual.SetContent(&swap_chain).ok()?;
            comp_target.SetRoot(&comp_visual).ok()?;
            comp_device.Commit().ok()?;
        }

        // Create descriptor heap for render target views
        let rtv_desc_heap = unsafe {
            let desc = D3D12_DESCRIPTOR_HEAP_DESC {
                r#type: D3D12_DESCRIPTOR_HEAP_TYPE::D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
                num_descriptors: NUM_OF_FRAMES as _,
                flags: D3D12_DESCRIPTOR_HEAP_FLAGS::D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
                node_mask: 0,
            };
            let mut ptr: Option<ID3D12DescriptorHeap> = None;
            device
                .CreateDescriptorHeap(&desc, &ID3D12DescriptorHeap::IID, ptr.set_abi())
                .and_some(ptr)
        }?;

        // Create resource per frame
        let mut descriptor = unsafe { rtv_desc_heap.GetCPUDescriptorHandleForHeapStart() };
        let rtv_desc_size = unsafe {
            device.GetDescriptorHandleIncrementSize(
                D3D12_DESCRIPTOR_HEAP_TYPE::D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            ) as usize
        };
        let resources = (0..NUM_OF_FRAMES)
            .map(|i| {
                let resource = unsafe {
                    let mut ptr: Option<ID3D12Resource> = None;
                    swap_chain
                        .GetBuffer(i as _, &ID3D12Resource::IID, ptr.set_abi())
                        .and_some(ptr)
                }?;

                unsafe {
                    // let desc = D3D12_TEX2D_RTV {
                    //     Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    //     u: D3D12_RTV_DIMENSION_UNKNOWN as _,
                    //     ViewDimension: 0,
                    // };
                    device.CreateRenderTargetView(&resource, 0 as _, &descriptor);
                    descriptor.ptr += rtv_desc_size;
                }

                Ok(resource)
            })
            .collect::<Result<Vec<_>, windows::ErrorCode>>()?
            .try_into()
            .expect("Unable to create resources");

        // Create root signature
        let root_signature = unsafe {
            let root = {
                let mut blob: Option<ID3DBlob> = None;
                let mut error: Option<ID3DBlob> = None;

                let desc = D3D12_ROOT_SIGNATURE_DESC {
                    num_parameters: 0,
                    p_parameters: null_mut() as _,
                    num_static_samplers: 0,
                    p_static_samplers: null_mut() as _,
                    flags: D3D12_ROOT_SIGNATURE_FLAGS::D3D12_ROOT_SIGNATURE_FLAG_NONE,
                };
                D3D12SerializeRootSignature(
                    &desc,
                    D3D_ROOT_SIGNATURE_VERSION::D3D_ROOT_SIGNATURE_VERSION_1_0,
                    &mut blob as _,
                    &mut error as _,
                )
                .and_then(|| {
                    if error.is_none() {
                        blob.unwrap()
                    } else {
                        panic!("Root signature failed, error blob contains the error")
                    }
                })
            }?;

            let mut ptr: Option<ID3D12RootSignature> = None;
            device
                .CreateRootSignature(
                    0,
                    root.GetBufferPointer(),
                    root.GetBufferSize(),
                    &ID3D12RootSignature::IID,
                    ptr.set_abi(),
                )
                .and_some(ptr)
        }?;

        // Create direct command list
        let list_graphics_direct = unsafe {
            let mut ptr: Option<ID3D12GraphicsCommandList> = None;
            device
                .CreateCommandList(
                    0,
                    D3D12_COMMAND_LIST_TYPE::D3D12_COMMAND_LIST_TYPE_DIRECT,
                    &allocator,
                    None,
                    &ID3D12GraphicsCommandList::IID,
                    ptr.set_abi(),
                )
                .and_then(|| {
                    let ptr = ptr.unwrap();
                    ptr.Close().unwrap();
                    ptr
                })
        }?;

        // let mut inputElementDescs = [
        //     D3D12_INPUT_ELEMENT_DESC {
        //         semantic_name: PSTR("POSITION\0".as_ptr() as _),
        //         semantic_index: 0,
        //         format: DXGI_FORMAT::DXGI_FORMAT_R32G32B32_FLOAT,
        //         input_slot: 0,
        //         instance_data_step_rate: 0,
        //         input_slot_class:
        //             D3D12_INPUT_CLASSIFICATION::D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
        //         aligned_byte_offset: 0,
        //     },
        //     D3D12_INPUT_ELEMENT_DESC {
        //         semantic_name: PSTR("COLOR\0".as_ptr() as _),
        //         semantic_index: 0,
        //         format: DXGI_FORMAT::DXGI_FORMAT_R32G32B32A32_FLOAT,
        //         input_slot: 0,
        //         instance_data_step_rate: 0,
        //         input_slot_class:
        //             D3D12_INPUT_CLASSIFICATION::D3D12_INPUT_CLASSIFICATION_PER_VERTEX_DATA,
        //         aligned_byte_offset: 12,
        //     },
        // ];

        // let mut pso_desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC::default();
        // pso_desc.input_layout = D3D12_INPUT_LAYOUT_DESC {
        //     num_elements: inputElementDescs.len() as u32,
        //     p_input_element_descs: inputElementDescs.as_mut_ptr(),
        // };
        // pso_desc.p_root_signature = Some(root_signature.clone());

        // // CD3DX12_RASTERIZER_DESC( CD3DX12_DEFAULT )
        // pso_desc.rasterizer_state = D3D12_RASTERIZER_DESC {
        //     fill_mode: D3D12_FILL_MODE::D3D12_FILL_MODE_SOLID,
        //     cull_mode: D3D12_CULL_MODE::D3D12_CULL_MODE_BACK,
        //     front_counter_clockwise: BOOL(0),
        //     depth_bias: D3D12_DEFAULT_DEPTH_BIAS,
        //     depth_bias_clamp: D3D12_DEFAULT_DEPTH_BIAS_CLAMP,
        //     slope_scaled_depth_bias: D3D12_DEFAULT_SLOPE_SCALED_DEPTH_BIAS,
        //     depth_clip_enable: BOOL(1),
        //     multisample_enable: BOOL(0),
        //     antialiased_line_enable: BOOL(0),
        //     forced_sample_count: 0,
        //     conservative_raster:
        //         D3D12_CONSERVATIVE_RASTERIZATION_MODE::D3D12_CONSERVATIVE_RASTERIZATION_MODE_OFF,
        // };

        // // CD3DX12_BLEND_DESC(D3D12_DEFAULT)
        // pso_desc.blend_state = D3D12_BLEND_DESC {
        //     alpha_to_coverage_enable: BOOL(0),
        //     independent_blend_enable: BOOL(0),
        //     render_target: // TODO: Windows RS docs says this is not yet supported "render_target: NOT_YET_SUPPORTED_TYPE" https://microsoft.github.io/windows-docs-rs/doc/bindings/windows/win32/direct3d12/struct.D3D12_BLEND_DESC.html
        // };

        // pso_desc.DepthStencilState.DepthEnable = FALSE;
        // pso_desc.DepthStencilState.StencilEnable = FALSE;
        // pso_desc.SampleMask = UINT_MAX;
        // pso_desc.PrimitiveTopologyType = D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE;
        // pso_desc.NumRenderTargets = 1;
        // pso_desc.RTVFormats[0] = DXGI_FORMAT_R8G8B8A8_UNORM;
        // pso_desc.SampleDesc.Count = 1;
        // ThrowIfFailed(m_device->CreateGraphicsPipelineState(&psoDesc, IID_PPV_ARGS(&m_pipelineState)));
        // D3DCompile(p_src_data, src_data_size, p_source_name, p_defines, p_include, p_entrypoint, p_target, flags1, flags2, pp_code, pp_error_msgs)

        // let pipeline_state = unsafe {
        //     let mut ptr: Option<ID3D12PipelineState> = None;
        //     device
        //         .CreateGraphicsPipelineState(&pso_desc, &ID3D12PipelineState::IID, ptr.set_abi())
        //         .and_some(ptr)
        // }
        // .expect("Unable to create pipeline state");

        Ok(Window {
            hwnd,
            factory,
            adapter,
            device,
            queue,
            allocator,
            comp_device,
            swap_chain,
            comp_target,
            comp_visual,
            rtv_desc_heap,
            rtv_desc_size,
            resources,
            root_signature,
            list_graphics_direct,
            // pipeline_state,
        })
    }

    fn populate_command_list(&mut self) -> ::windows::Result<()> {
        unsafe {
            // Get the current backbuffer on which to draw
            let current_frame = self.swap_chain.GetCurrentBackBufferIndex() as usize;
            let _current_resource = &self.resources[current_frame];
            let rtv = {
                let mut ptr = self.rtv_desc_heap.GetCPUDescriptorHandleForHeapStart();
                ptr.ptr += self.rtv_desc_size * current_frame;
                ptr
            };

            // Reset allocator
            self.allocator.Reset().ok()?;

            // Reset list
            self.list_graphics_direct
                .Reset(&self.allocator, None)
                .ok()?;

            // Set root signature
            self.list_graphics_direct
                .SetGraphicsRootSignature(&self.root_signature);

            // Clear view
            self.list_graphics_direct.ClearRenderTargetView(
                rtv,
                [1.0f32, 0.2, 0.4, 0.5].as_ptr(),
                0,
                null_mut(),
            );

            // Close list
            self.list_graphics_direct.Close().ok()?;
            Ok(())
        }
    }

    pub fn render(&mut self) -> windows::Result<()> {
        self.populate_command_list()?;
        unsafe {
            let mut lists = [Some(self.list_graphics_direct.cast::<ID3D12CommandList>()?)];
            self.queue
                .ExecuteCommandLists(lists.len() as _, lists.as_mut_ptr());
            self.swap_chain.Present(1, 0).ok()?;
        }
        Ok(())
    }
}

/// Main message loop for the window
extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        static mut WINDOW: Option<Window> = None;
        match msg as i32 {
            WM_CREATE => {
                WINDOW = Some(Window::new(hwnd).unwrap());
                DefWindowProcA(hwnd, msg, wparam, lparam)
            }
            WM_PAINT => {
                if let Some(window) = WINDOW.as_mut() {
                    window.render().unwrap();
                }
                ValidateRect(hwnd, std::ptr::null());
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcA(hwnd, msg, wparam, lparam),
        }
    }
}

fn main() {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleA(PSTR(null_mut())));
        let cursor = LoadCursorA(HINSTANCE(0), PSTR(IDC_ARROW as _));
        let cls = WNDCLASSA {
            style: 0,
            lpfn_wnd_proc: Some(wndproc),
            h_instance: instance,
            lpsz_class_name: PSTR(b"CompositionCls\0".as_ptr() as _),
            cb_cls_extra: 0,
            cb_wnd_extra: 0,
            h_icon: HICON(0),
            h_cursor: cursor,
            hbr_background: HBRUSH(0),
            lpsz_menu_name: PSTR(null_mut()),
        };
        RegisterClassA(&cls);
        let hwnd = CreateWindowExA(
            WS_EX_NOREDIRECTIONBITMAP as _,
            PSTR(b"CompositionCls\0".as_ptr() as _),
            PSTR(b"Composition example\0".as_ptr() as _),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND(0),
            HMENU(0),
            instance,
            0 as _,
        );
        if hwnd == HWND(0) {
            panic!("Failed to create window");
        }

        let mut message = MSG::default();

        while GetMessageA(&mut message, HWND(0), 0, 0).into() {
            TranslateMessage(&mut message);
            DispatchMessageA(&mut message);
        }
    }
}
