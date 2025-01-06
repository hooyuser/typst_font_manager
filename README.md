# Typst Font Manager CLI

![Lines of code](https://tokei.rs/b1/github/hooyuser/typst_font_manager) ![](https://img.shields.io/github/repo-size/hooyuser/typst_font_manager?style=plastic
)

- [Font Configuration for Typst Projects](#font-configuration-for-typst-projects)
- [CLI Command Guide](#cli-command-guide)
- [GitHub CI Integration](#gitHub-ci-integration)

<a name="font-configuration-for-typst-projects"/>

## 📚 **Font Configuration for Typst Projects**

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

### **Font Configuration Rules**

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

<a name="cli-command-guide"/>

## 🛠️ **CLI Command Guide**

The following steps outline how to explicitly set up font dependencies for your local Typst project by strictly specifying the font variants you want to use.

---

#### **1 Identify Required Font Variants**  

- Start by determining the **exact font variants** your project needs.  
- A handy tool for this is **Tinymist**. Open the command palette (`Ctrl+Shift+P`) and search for `summary`.  
- Click on **`Typst: Show current document summary`**, and in the newly opened page, scroll down to view all fonts currently used in your Typst project.  
- Click the **"book" icon** in the top-right corner to access detailed information about each font, including:  
   - **Family Name**  
   - **Style**  
   - **Weight**  
   - **First Occurrence Row Number**  
   - **Path to Font File**  
- In Typst, the combination of **family name**, **style**, and **weight** uniquely identifies a font variant. Use this information to select the precise variants for your project.

---

#### **2 Create a Font Library**  

- Create a dedicated **font library** to store all fonts required across your Typst projects.  
- While you can choose the **system font directory**, it’s often easier to maintain a separate directory for clarity.  
- For example, create a directory at `/Users/goodguy/font_lib`
- Copy all the fonts you plan to use into this directory.
- Organize fonts by placing each family into its own subdirectory.

---


#### **3 Verify Fonts in the Library**  

- Remove any existing font files from your Typst project (consider backing them up first).  
- Run the following command:  
   ```sh
   ./typfont check -l "/Users/goodguy/font_lib"
   ```  
- This command will display:  
   - Fonts **required** by your project.  
   - Fonts **available** in your library.  
   - Fonts **missing** from your library.  
- If all required fonts are present in the library, you’re good to proceed.

---

#### **4 Update Project Fonts**  

- Run the following command to copy the required fonts from the library to your project’s configured font directory:  
   ```sh
   ./typfont update -l "/Users/goodguy/font_lib"
   ```  
- This ensures only the required fonts are copied to your project.

---

#### **5 Final Verification**  

- Run the check command again to ensure nothing is missing:  
   ```sh
   ./typfont check -l "/Users/goodguy/font_lib"
   ```  
- The output should confirm that **Missing fonts (total 0)**.

---

#### **6 Ensure Explicit Font Management**  

To minimize unexpected font fallback and ensure strict font management, use the following command when compiling your Typst document:  
```sh
typst compile foo.typ foo.pdf --ignore-system-fonts --font-path fonts
```

Additionally, add this line to your Typst file:  
```typst
#set text(fallback: false)
```

This prevents Typst from falling back to unintended font variants, ensuring consistent and reproducible results across your project.

---

By following these steps, you'll have precise control over font management in your Typst projects, minimizing font-related issues and ensuring clarity in your setup.

<a name="gitHub-ci-integration"/>

## 🚀 **GitHub CI Integration**

If you want to avoid tracking numerous font files in your Typst project's GitHub repository, this CLI tool can help streamline the process.

### **1 Generate Font Library Information**  
- Use the following command to generate font library information:  
   ```sh
   ./typfont
   ```  

---

### **2 Create and Push Font Library Repository**  
- Make your **local font library** a Git repository.  
- Push your local font library to a remote GitHub repository.  
- Assume the following:  
   - **GitHub Username:** `gooduser`  
   - **Font Library Repository Name:** `font_lib`

---

### **3 Set Up GitHub Actions Workflow**  
- In your **GitHub Actions** workflow, download the **latest release** of this CLI tool.  
- Run the following command to download fonts from your remote font library into your GitHub Actions worker:  
   ```sh
   ./typfont update -l "gooduser/font_lib" -g
   ```  

- For reference, you can check one of my CI workflow examples:  
   [Example CI Workflow](https://github.com/hooyuser/functional_analysis/blob/main/.github/workflows/generate_release_pdf.yml)





