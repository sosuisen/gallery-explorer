//! Dragging files and folders from an element to other apps.
//!
//! Added in GalleryExplorer. Setting `DragStorageItems` on an element sets `CanDrag`. When a
//! drag starts, the element puts its files and folders in the drag as storage items.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use windows::ApplicationModel::DataTransfer::DataPackage;
use windows::Storage::{IStorageItem, StorageFile, StorageFolder};
use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};
use windows_collections_03::IIterable;
use windows_core::Interface;

use super::bindings::{
    DataPackageOperation, DispatcherQueue, DispatcherQueueHandler, DispatcherQueuePriority,
    DragOperationDeferral, DragStartingEventArgs, IUIElement, UIElement,
};

/// The drag of one element. Dropping it stops the element from handling drags.
pub(super) struct DragSource {
    paths: Rc<RefCell<Vec<PathBuf>>>,
    _starting: windows_core::EventRevoker,
}

impl DragSource {
    pub(super) fn start(element: &UIElement, paths: Vec<PathBuf>) -> windows_core::Result<Self> {
        let element = element.cast::<IUIElement>()?;
        element.SetCanDrag(true)?;
        let paths = Rc::new(RefCell::new(paths));
        let shared = Rc::clone(&paths);
        let starting = element.DragStarting(move |_, args| {
            if let Some(args) = args.as_ref() {
                // A failed drag has no one to report to. The user sees that nothing was dropped.
                _ = fill_drag(args, shared.borrow().clone());
            }
        })?;
        Ok(Self {
            paths,
            _starting: starting,
        })
    }

    pub(super) fn set_paths(&self, paths: Vec<PathBuf>) {
        *self.paths.borrow_mut() = paths;
    }

    pub(super) fn stop(element: &UIElement) -> windows_core::Result<()> {
        element.cast::<IUIElement>()?.SetCanDrag(false)
    }
}

/// Puts the files and folders in the drag. The target picks copy, move or link, with the same
/// keys as in File Explorer.
///
/// The storage items can only be made asynchronously, so the drag waits on a deferral. They are
/// made on another thread, because a wait on the UI thread could block the drag.
fn fill_drag(args: &DragStartingEventArgs, paths: Vec<PathBuf>) -> windows_core::Result<()> {
    args.SetAllowedOperations(
        DataPackageOperation::Copy | DataPackageOperation::Move | DataPackageOperation::Link,
    )?;
    // The crate's IInspectable and the one of the windows crate are the same COM object.
    let package = unsafe { <DataPackage as windows::core::Interface>::from_raw(args.Data()?.into_raw()) };
    let deferral = args.GetDeferral()?;
    let dispatcher = DispatcherQueue::GetForCurrentThread()?;
    std::thread::spawn(move || {
        // A data package is agile, so the thread can fill it.
        _ = set_storage_items(&package, &paths);
        complete_on(&dispatcher, deferral);
    });
    Ok(())
}

fn set_storage_items(package: &DataPackage, paths: &[PathBuf]) -> windows::core::Result<()> {
    // SAFETY: each success is paired with CoUninitialize on the same thread.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.ok()?;
    let result = storage_items(paths).and_then(|items| {
        package.SetStorageItemsReadOnly(&IIterable::<IStorageItem>::from(items.into_iter().map(Some).collect::<Vec<_>>()))
    });
    unsafe { CoUninitialize() };
    result
}

fn storage_items(paths: &[PathBuf]) -> windows::core::Result<Vec<IStorageItem>> {
    paths
        .iter()
        .map(|path| {
            use windows::core::Interface as _;
            let name = windows::core::HSTRING::from(path.as_path());
            if path.is_dir() {
                StorageFolder::GetFolderFromPathAsync(&name)?.join()?.cast()
            } else {
                StorageFile::GetFileFromPathAsync(&name)?.join()?.cast()
            }
        })
        .collect()
}

/// Completes the deferral on the UI thread, where WinUI made it.
fn complete_on(dispatcher: &DispatcherQueue, deferral: DragOperationDeferral) {
    let handler = DispatcherQueueHandler::new(move || {
        _ = deferral.Complete();
    });
    _ = dispatcher.TryEnqueueWithPriority(DispatcherQueuePriority::Normal, &handler);
}
