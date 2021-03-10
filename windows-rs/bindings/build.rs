fn main() {
    windows::build!(
        windows::win32::direct3d12::*,
        windows::win32::dxgi::*,
        windows::win32::gdi::ValidateRect,
        windows::win32::menus_and_resources::{LoadCursorA, HMENU},
        windows::win32::system_services::{
            GetModuleHandleA, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HINSTANCE, IDC_ARROW, LRESULT, WM_CREATE,
            WM_DESTROY, WM_PAINT, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WS_EX_NOREDIRECTIONBITMAP
        },
        windows::win32::windows_and_messaging::{
            CreateWindowExA, DefWindowProcA, DispatchMessageA, GetMessageA, PostQuitMessage,
            TranslateMessage,
            RegisterClassA, HWND, LPARAM, MSG, WNDCLASSA, WPARAM,
        },
        windows::win32::direct_composition::{IDCompositionDevice, IDCompositionTarget, IDCompositionVisual, DCompositionCreateDevice}
    );
}

/*
fn main() {
    windows::build!(
        // windows::win32::direct3d11::*,
        // windows::win32::dxgi::*,
        windows::win32::gdi::*,
        windows::win32::windows_and_messaging::*,
        windows::win32::windows_programming::*,
        windows::win32::system_services::*,
        windows::win32::menus_and_resources::*
    );
}
*/
