# Typst Font Manager CLI

![Lines of code](https://tokei.rs/b1/github/hooyuser/typst_font_manager) ![](https://img.shields.io/github/repo-size/hooyuser/typst_font_manager?style=plastic
)

### 📚 **Font Configuration for Typst Projects**

To use this font manager CLI tool with your Typst project, place a font configuration file named `font_config.toml` in the same directory as your Typst file `*.typ`.

Below is an example `font_config.toml` file that explicitly specifies **5 fonts** from **3 font families**:  
- **Noto Sans** (3 variants)  
- **Noto Sans Display** (1 variant)  
- **STIXTwoText** (1 variant)  

```toml
font_dir = "fonts"

[[fonts]]
family_name = "Noto Sans"
style = "Normal"
weight = [400, 600, 700]

[[fonts]]
family_name = "Noto Sans Display"
style = "Normal"
weight = 500

[[fonts]]
family_name = "STIXTwoText"
style = "Italic"
weight = 400
```
### 🛠️ **Font Configuration Rules**

1. **Font Directory:**  
   - Use `font_dir = "fonts"` to specify the subdirectory where font files are stored.  
   - If omitted, the default directory is `fonts`.

2. **Explicit Font Variants:**  
   - The configuration explicitly specifies font variants instead of relying on a font family to map multiple variants automatically.

3. **Font Family Naming:**  
   - Ensure that the `family_name` matches the names shown by the `typst fonts` command.

4. **Weight Specification:**  
   - Use an array like `[400, 600, 700]` to specify multiple font weights explicitly.

5. **Default Style and Weight:**  
   - If `style` is omitted, the default is `"Normal"`. No fuzzy matching is applied.  
   - If `weight` is omitted, the default is `400`.

