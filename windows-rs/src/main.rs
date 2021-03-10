use std::{convert::TryInto, mem, ptr, rc::Weak};
use std::{ffi::c_void, ptr::null_mut};

use bindings::{
    windows::win32::direct3d11::*, windows::win32::direct3d12::*,
    windows::win32::direct_composition::*, windows::win32::dxgi::*, windows::win32::gdi::*,
    windows::win32::menus_and_resources::*, windows::win32::system_services::*,
    windows::win32::windows_and_messaging::*, windows::*,
};
use ptr::null;
use windows::{Abi, IUnknown, Interface};

const NUM_OF_FRAMES: usize = 2;

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
    // list: ID3D12GraphicsCommandList,
    rtv_desc_heap: ID3D12DescriptorHeap,
    rtv_desc_size: usize,
    // desc_size: Option<u32>,
    resources: [ID3D12Resource; NUM_OF_FRAMES],
    root_signature: ID3D12RootSignature,
    list_graphics_direct: ID3D12GraphicsCommandList,
    // resources: Vec<ID3D12Resource>,
    // pipeline_state: ID3D12PipelineState,
    // root_signature: ID3D12RootSignature,
}

impl Window {
    pub fn new(hwnd: HWND) -> windows::Result<Self> {
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
                Some(adapter.clone().into()),
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
        }
        .expect("Unable to create composition device");

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
                .CreateSwapChainForComposition(queue.clone(), &desc, None, &mut ptr)
                .and_some(ptr)
        }?
        .cast::<IDXGISwapChain3>()?;

        // Create IDCompositionTarget for the window
        let comp_target = unsafe {
            let mut ptr = None;
            comp_device
                .CreateTargetForHwnd(hwnd, BOOL(1), &mut ptr)
                .and_some(ptr)
        }
        .expect("Unable to composition target");

        // Create IDCompositionVisual for the window
        let comp_visual = unsafe {
            let mut ptr = None;
            comp_device.CreateVisual(&mut ptr).and_some(ptr)
        }
        .expect("Unable to composition visual");

        // Set swap_chain and the root visual and commit
        unsafe {
            comp_visual.SetContent(swap_chain.clone()).unwrap();
            comp_target.SetRoot(comp_visual.clone()).unwrap();
            comp_device.Commit().unwrap();
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
        }
        .expect("Unable to create heap descriptor thing");

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
                }
                .expect("Unable to create resource");

                unsafe {
                    // let desc = D3D12_TEX2D_RTV {
                    //     Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    //     u: D3D12_RTV_DIMENSION_UNKNOWN as _,
                    //     ViewDimension: 0,
                    // };
                    device.CreateRenderTargetView(resource.clone(), 0 as _, descriptor.clone());
                    descriptor.ptr += rtv_desc_size;
                }

                resource
            })
            .collect::<Vec<_>>()
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
            }
            .expect("Root signature serialization failed");

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
        }
        .expect("Unable to create root signature");

        // Create direct command list
        let list_graphics_direct = unsafe {
            let mut ptr: Option<ID3D12GraphicsCommandList> = None;
            device
                .CreateCommandList(
                    0,
                    D3D12_COMMAND_LIST_TYPE::D3D12_COMMAND_LIST_TYPE_DIRECT,
                    allocator.clone(),
                    None,
                    &ID3D12GraphicsCommandList::IID,
                    ptr.set_abi(),
                )
                .and_then(|| {
                    let ptr = ptr.unwrap();
                    ptr.Close().unwrap();
                    ptr
                })
        }
        .expect("Unable to create command list");

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
        })
    }

    fn populate_command_list(&mut self) {
        unsafe {
            // Get the current backbuffer on which to draw
            let current_frame = unsafe { self.swap_chain.GetCurrentBackBufferIndex() as usize };
            let current_resource = &self.resources[current_frame];
            let current_resource_desc = {
                let mut ptr = self.rtv_desc_heap.GetCPUDescriptorHandleForHeapStart();
                ptr.ptr += self.rtv_desc_size * current_frame;
                ptr
            };

            self.allocator.Reset().unwrap();
            self.list_graphics_direct
                .Reset(self.allocator.clone(), None)
                .unwrap();
            self.list_graphics_direct
                .SetGraphicsRootSignature(self.root_signature.clone());

            self.list_graphics_direct.ClearRenderTargetView(
                current_resource_desc,
                [1.0f32, 0.2, 0.4, 0.5].as_ptr(),
                0,
                null_mut(),
            );

            self.list_graphics_direct.Close().unwrap();
        }
    }

    pub fn render(&mut self) {
        self.populate_command_list();
        unsafe {
            let mut lists = [Some(
                self.list_graphics_direct
                    .cast::<ID3D12CommandList>()
                    .unwrap(),
            )];
            self.queue
                .ExecuteCommandLists(lists.len() as _, lists.as_mut_ptr());
            self.swap_chain.Present(1, 0).unwrap();
        }
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
                    window.render();
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
