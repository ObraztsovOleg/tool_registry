use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};
use tool_interface::{Tool, CreateToolFn};
use libloading::Library;


pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
    loaded_libraries: HashMap<PathBuf, (SystemTime, Library)>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            loaded_libraries: HashMap::new(),
        }
    }

    pub fn tools_specs(&self) -> Vec<serde_json::Value> {
        self.tools.iter().map(|(_, tool)| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": tool.name(),
                    "description": tool.description(),
                    "parameters": tool.parameters()
                }
            })
        }).collect()
    }

    pub fn load_from_dir(&mut self, dir_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let dir_entries = std::fs::read_dir(dir_path)?;
        
        for entry in dir_entries {
            let entry = entry?;
            let path = entry.path();
            
            if is_shared_library(&path) {
                self.load_library(&path)?;
            }
        }
        
        Ok(())
    }

    pub fn get_tool(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    fn load_library(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let metadata = std::fs::metadata(path)?;
        let modified = metadata.modified()?;
        
        if let Some((prev_modified, _)) = self.loaded_libraries.get(path) {
            if &modified <= prev_modified {
                return Ok(());
            }
            // Unload old version if we're reloading
            self.unload_library(path);
        }
        
        unsafe {
            let lib = Library::new(path)?;
            let constructor: libloading::Symbol<CreateToolFn> = lib.get(b"create_tool")?;

            let tool_ptr = constructor();
            let tool: Box<dyn Tool> = Box::from_raw(tool_ptr);
            let name = tool.name().to_string();
            
            self.tools.insert(name, tool);
            self.loaded_libraries.insert(path.to_path_buf(), (modified, lib));
        }
        
        Ok(())
    }

    fn unload_library(&mut self, path: &Path) {
        if let Some((_, library)) = self.loaded_libraries.remove(path) {
            // Find all tools from this library and remove them
            let tools_to_remove: Vec<String> = self.tools.iter()
                .filter(|(_, tool)| {
                    // This is a simplistic approach - you might need a better way
                    // to associate tools with their libraries
                    true
                })
                .map(|(name, _)| name.clone())
                .collect();
            
            for tool_name in tools_to_remove {
                self.tools.remove(&tool_name);
            }
            
            // Library will be dropped here
        }
    }
}

fn is_shared_library(path: &Path) -> bool {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
    cfg_if::cfg_if! {
        if #[cfg(target_os = "windows")] {
            ext.eq_ignore_ascii_case("dll")
        } else if #[cfg(target_os = "macos")] {
            ext.eq_ignore_ascii_case("dylib")
        } else {
            ext.eq_ignore_ascii_case("so")
        }
    }
}