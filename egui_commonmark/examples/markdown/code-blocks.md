# Code blocks

```rs
use egui_commonmark::*;
let markdown =
r"# Hello world

* A list
* [ ] Checkbox
";

let mut cache = CommonMarkCache::default();
CommonMarkViewer::new("viewer").show(ui, &mut cache, markdown);
```

```toml
egui_commonmark = "0.10"
image = { version = "0.24", default-features = false, features = ["png"] }
```

Code fences with an unrecognised or missing info string will fall back to `syntect` plain text.

```powershell
# Get a list of all running services starting with "Win"
Get-Service -Name "Win*" | Select-Object Name, Status, DisplayName
```

- ```rs
  let x = 3.14;
  ```
- Code blocks can be in lists too :)


More content...

    Inline code blocks are supported if you for some reason need them
