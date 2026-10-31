//! Windows printer access for direct (RAW / ESC-POS) receipt printing.
//!
//! Normal printing goes through the Windows print dialog from the web view
//! and works with any installed printer. This module adds silent printing
//! to thermal printers by sending ESC/POS bytes straight to the spooler.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrinterInfo {
    pub name: String,
    pub is_default: bool,
}

#[cfg(windows)]
mod imp {
    use super::PrinterInfo;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Graphics::Printing::{
        ClosePrinter, EndDocPrinter, EndPagePrinter, EnumPrintersW, GetDefaultPrinterW, OpenPrinterW, StartDocPrinterW,
        StartPagePrinter, WritePrinter, DOC_INFO_1W, PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_4W,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// # Safety
    /// `p` must be null or point to a NUL-terminated UTF-16 string.
    unsafe fn from_wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut len = 0;
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }

    pub fn default_printer() -> Option<String> {
        let mut size: u32 = 0;
        unsafe {
            GetDefaultPrinterW(std::ptr::null_mut(), &mut size);
            if size == 0 {
                return None;
            }
            let mut buf = vec![0u16; size as usize];
            if GetDefaultPrinterW(buf.as_mut_ptr(), &mut size) == 0 {
                return None;
            }
            Some(from_wide(buf.as_ptr()))
        }
    }

    pub fn list() -> Result<Vec<PrinterInfo>, String> {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let mut needed: u32 = 0;
        let mut returned: u32 = 0;
        let default = default_printer();
        unsafe {
            EnumPrintersW(flags, std::ptr::null(), 4, std::ptr::null_mut(), 0, &mut needed, &mut returned);
            if needed == 0 {
                return Ok(Vec::new());
            }
            // u64 storage keeps the PRINTER_INFO_4W records correctly aligned.
            let mut buf = vec![0u64; (needed as usize).div_ceil(8)];
            if EnumPrintersW(
                flags,
                std::ptr::null(),
                4,
                buf.as_mut_ptr() as *mut u8,
                needed,
                &mut needed,
                &mut returned,
            ) == 0
            {
                return Err("Windows could not list the installed printers.".into());
            }
            let infos = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, returned as usize);
            Ok(infos
                .iter()
                .map(|i| {
                    let name = from_wide(i.pPrinterName);
                    PrinterInfo { is_default: default.as_deref() == Some(name.as_str()), name }
                })
                .collect())
        }
    }

    pub fn print_raw(printer: &str, doc_name: &str, data: &[u8]) -> Result<(), String> {
        let name = wide(printer);
        let mut doc = wide(doc_name);
        let mut datatype = wide("RAW");
        unsafe {
            let mut handle: HANDLE = std::ptr::null_mut();
            if OpenPrinterW(name.as_ptr(), &mut handle, std::ptr::null()) == 0 {
                return Err(format!("Printer '{printer}' was not found or is offline."));
            }
            let info = DOC_INFO_1W {
                pDocName: doc.as_mut_ptr(),
                pOutputFile: std::ptr::null_mut(),
                pDatatype: datatype.as_mut_ptr(),
            };
            let result = (|| {
                if StartDocPrinterW(handle, 1, &info) == 0 {
                    return Err(format!("Printer '{printer}' refused the print job."));
                }
                if StartPagePrinter(handle) == 0 {
                    EndDocPrinter(handle);
                    return Err("Could not start printing.".to_string());
                }
                let mut written: u32 = 0;
                let ok = WritePrinter(handle, data.as_ptr() as *const _, data.len() as u32, &mut written);
                EndPagePrinter(handle);
                EndDocPrinter(handle);
                if ok == 0 || written as usize != data.len() {
                    return Err("The printer did not accept all data. Check the paper and the connection.".into());
                }
                Ok(())
            })();
            ClosePrinter(handle);
            result
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::PrinterInfo;

    pub fn list() -> Result<Vec<PrinterInfo>, String> {
        Ok(Vec::new())
    }

    pub fn print_raw(_printer: &str, _doc: &str, _data: &[u8]) -> Result<(), String> {
        Err("Direct printing is only available on Windows.".into())
    }
}

pub use imp::{list, print_raw};
