extern crate failure;
extern crate minidom;
extern crate quick_xml;
extern crate single;

#[macro_use]
extern crate failure_derive;

use failure::Error;
use minidom::element::Node;
use minidom::Element;
use quick_xml::Reader;
use single::Single;
use std::collections::HashSet;
use std::fs::File;
use std::io::BufWriter;
use std::io::Write;

#[derive(Debug, Fail)]
enum ParseError {
    #[fail(display = "no child elements match condition: {}", cond)]
    MissingChild { cond: String },

    #[fail(display = "too many child elements match condition: {}", cond)]
    ExtraChildren { cond: String },

    #[fail(display = "missing attribute: {}", attr)]
    MissingAttr { attr: String },
}

fn only_child_where<'a, F: for<'r> FnMut(&'r &Element) -> bool, G: FnOnce() -> String>(
    el: &'a Element,
    cond: F,
    cond_string: G,
) -> Result<&'a Element, ParseError> {
    match el.children().filter(cond).single() {
        Ok(c) => Ok(c),
        Err(single::Error::NoElements) => Err(ParseError::MissingChild {
            cond: cond_string(),
        }),
        Err(single::Error::MultipleElements) => Err(ParseError::ExtraChildren {
            cond: cond_string(),
        }),
    }
}

fn only_child_named<'a>(el: &'a Element, name: &str) -> Result<&'a Element, ParseError> {
    only_child_where(el, |c| c.name() == name, || format!("named == {}", name))
}

fn get_attr<'a>(el: &'a Element, attr: &str) -> Result<&'a str, ParseError> {
    match el.attr(attr) {
        Some(s) => Ok(s),
        None => Err(ParseError::MissingAttr {
            attr: attr.to_owned(),
        }),
    }
}

fn gather_all_inner_text(el: &Element, output: &mut String) {
    for node in el.nodes() {
        match node {
            Node::Element(e) => {
                gather_all_inner_text(&e, output);
            }
            Node::Text(s) => {
                output.push_str(&s);
            }
            Node::Comment(_) => {}
        }
    }
}

fn get_all_classes<'a>(index_root: &'a Element) -> impl Iterator<Item = (String, String)> + 'a {
    index_root.children().filter_map(|compound| {
        if compound.attr("kind") == Some("class") {
            if let (Ok(name), Ok(refid)) = (
                only_child_named(compound, "name"),
                get_attr(compound, "refid"),
            ) {
                return Some((name.text(), refid.to_owned()));
            }
        }
        return None;
    })
}

fn generate_constructor_wrappers<W: Write>(
    class_root: &Element,
    hxx_file: &mut W,
    cxx_file: &mut W,
) -> Result<(), Error> {
    let compounddef = only_child_named(class_root, "compounddef")?;
    let name = only_child_named(compounddef, "compoundname")?.text();
    let constructor_name = format!("{0}::{0}", name);
    let sectiondef = only_child_where(
        compounddef,
        |c| c.name() == "sectiondef" && c.attr("kind") == Some("public-func"),
        || "sectiondef[kind=public-func]".to_owned(),
    )?;
    for memberdef in sectiondef.children() {
        let definition = only_child_named(memberdef, "definition")?.text();
        if definition != constructor_name {
            continue;
        }
        let params = memberdef
            .children()
            .filter(|c| c.name() == "param")
            .collect::<Vec<_>>();
        let num_required_params = params
            .iter()
            .map(|c| {
                if only_child_named(c, "defval").is_err() {
                    1
                } else {
                    0
                }
            })
            .sum::<usize>();
        for num_params in num_required_params..params.len() {
            if num_params == 0 {
                writeln!(
                    cxx_file,
                    "{0}* construct_{0}() {{ return new {0}(); }}",
                    name
                )?;
                writeln!(hxx_file, "{0}* construct_{0}();", name)?;
            } else {
                let mut constructor_suffix = String::new();
                for (i, p) in params.iter().enumerate() {
                    if i < num_params {
                        if i != 0 {
                            constructor_suffix.push_str("_and_");
                        }
                        let declname = only_child_named(p, "declname")?;
                        constructor_suffix.push_str(&declname.text());
                    }
                }

                let mut taken_params = String::new();
                for (i, p) in params.iter().enumerate() {
                    if i < num_params {
                        if i != 0 {
                            taken_params.push_str(", ");
                        }
                        gather_all_inner_text(only_child_named(p, "type")?, &mut taken_params);
                        taken_params.push(' ');
                        let declname = only_child_named(p, "declname")?;
                        taken_params.push_str(&declname.text());
                    }
                }

                let mut passed_params = String::new();
                for (i, p) in params.iter().enumerate() {
                    if i < num_params {
                        if i != 0 {
                            passed_params.push_str(", ");
                        }
                        let declname = only_child_named(p, "declname")?;
                        passed_params.push_str(&declname.text());
                    }
                }

                writeln!(
                    cxx_file,
                    "{0}* construct_{0}_from_{1}({2}) {{ return new {0}({3}); }}",
                    name, constructor_suffix, taken_params, passed_params
                )?;
                writeln!(
                    hxx_file,
                    "{0}* construct_{0}_from_{1}({2});",
                    name, constructor_suffix, taken_params
                )?;
            }
        }
    }
    Ok(())
}

pub fn generate_wrapper(
    output_header: &str,
    output_source: &str,
    xml_path: &str,
) -> Result<(), Error> {
    let index_path = format!("{}/index.xml", xml_path);
    println!("index_path = {:?}", index_path);
    let index_root = Element::from_reader(&mut Reader::from_file(&index_path)?)?;

    let mut hxx_stream = BufWriter::new(File::create(output_header)?);
    let mut cxx_stream = BufWriter::new(File::create(output_source)?);
    writeln!(hxx_stream, "#include \"wx/wx.h\"")?;
    writeln!(cxx_stream, "#include \"{}\"", output_header)?;

    let mut class_blocklist: HashSet<String> = HashSet::new();
    let blocklist = r"
wxThreadHelper
wxThread
wxTextOutputStream
wxSpinEvent
wxDropTarget
wxScopedPtr
wxChoice
wxComboBox
wxCursor
wxDataInputStream
wxEvent
wxListBox
wxProcess
wxRadioBox
wxDataOutputStream
wxInitializer
wxMenuBar
wxAccessible
wxAnimation
wxAnimationCtrl
wxArrayStringProperty
wxAuiManager
wxAuiManagerEvent
wxAuiMDIChildFrame
wxAuiMDIClientWindow
wxAuiMDIParentFrame
wxAuiNotebook
wxAuiNotebookEvent
wxAuiToolBar
wxAuiToolBarEvent
wxLogStream
wxLogStderr
wxMessageOutputStderr
wxBufferedInputStream
wxAutomationObject
wxBannerWindow
wxBitmapComboBox
wxBitmapToggleButton
wxBookCtrlBase
wxBookCtrlEvent
wxBoolProperty
wxBufferedDC
wxBusyInfo
wxCalculateLayoutEvent
wxCalendarCtrl
wxCalendarDateAttr
wxCollapsiblePane
wxColourDialog
wxColourPickerCtrl
wxColourProperty
wxComboCtrl
wxCommand
wxCommandLinkButton
wxCommandProcessor
wxConfigBase
wxContextHelp
wxContextHelpButton
wxConvAuto
wxCursorProperty
wxDatagramSocket
wxDataViewBitmapRenderer
wxDataViewChoiceByIndexRenderer
wxDataViewChoiceRenderer
wxDataViewColumn
wxDataViewCtrl
wxDataViewCustomRenderer
wxDataViewDateRenderer
wxDataViewEvent
wxDataViewIconTextRenderer
wxDataViewIndexListModel
wxDataViewListCtrl
wxDataViewProgressRenderer
wxDataViewRenderer
wxDataViewSpinRenderer
wxDataViewTextRenderer
wxDataViewToggleRenderer
wxDataViewTreeCtrl
wxDataViewVirtualListModel
wxDatePickerCtrl
wxDateProperty
wxDebugReportUpload
wxDirFilterListCtrl
wxDirPickerCtrl
wxDirProperty
wxDisplay
wxDocChildFrame
wxDocManager
wxDocMDIChildFrame
wxDocParentFrame
wxDocTemplate
wxDocument
wxDragImage
wxDropSource
wxDynamicLibrary
wxEditableListBox
wxEditEnumProperty
wxEnumProperty
wxEOL
wxExtHelpController
wxFFile
wxFFileInputStream
wxFFileOutputStream
wxFFileStream
wxFile
wxFileConfig
wxFileHistory
wxFileName
wxFilePickerCtrl
wxFileProperty
wxFileSystemWatcherEvent
wxFindDialogEvent
wxFindReplaceData
wxFindReplaceDialog
wxFlagsProperty
wxFloatProperty
wxFontPickerCtrl
wxFontProperty
wxGBSizerItem
wxGenericAboutDialog
wxGenericDirCtrl
wxGenericProgressDialog
wxGLCanvas
wxGLContext
wxGraphicsGradientStop
wxGraphicsGradientStops
wxGridBagSizer
wxGridCellAttr
wxGridCellChoiceEditor
wxGridCellDateTimeRenderer
wxGridCellEnumEditor
wxGridCellEnumRenderer
wxGridCellFloatEditor
wxGridCellFloatRenderer
wxGridCellNumberEditor
wxGridCellTextEditor
wxGridEvent
wxGridRangeSelectEvent
wxGridSizeEvent
wxGridTableMessage
wxGridUpdateLocker
wxHashMap
wxHashSet
wxHeaderCtrl
wxHeaderCtrlEvent
wxHeaderCtrlSimple
wxHelpController
wxHelpControllerBase
wxHelpControllerHelpProvider
wxHScrolledWindow
wxHtmlColourCell
wxHtmlEasyPrinting
wxHtmlHelpController
wxHtmlHelpDialog
wxHtmlHelpFrame
wxHtmlHelpWindow
wxHtmlLinkInfo
wxHtmlListBox
wxHtmlModalHelp
wxHtmlPrintout
wxHtmlWidgetCell
wxHtmlWindow
wxHtmlWinParser
wxHVScrolledWindow
wxHyperlinkCtrl
wxImageFileProperty
wxImageList
wxInfoBar
wxIntProperty
wxJoystick
wxListbook
wxListCtrl
wxListEvent
wxListView
wxLongStringProperty
wxMediaCtrl
wxMediaEvent
wxMemoryInputStream
wxMemoryOutputStream
wxMetafile
wxMetafileDC
wxMiniFrame
wxMultiChoiceProperty
wxNotebook
wxNotificationMessage
wxNumericPropertyValidator
wxOwnerDrawnComboBox
wxPageSetupDialog
wxPGArrayEditorDialog
wxPGCell
wxPGChoiceEntry
wxPGChoices
wxPopupTransientWindow
wxPopupWindow
wxPreferencesEditor
wxPreviewCanvas
wxPreviewControlBar
wxPreviewFrame
wxPrintAbortDialog
wxPrintDialog
wxPrinter
wxPrintout
wxPrintPreview
wxProcessEvent
wxProgressDialog
wxPropertyCategory
wxPropertyGrid
wxPropertyGridConstIterator
wxPropertyGridEvent
wxPropertyGridIterator
wxPropertyGridManager
wxPropertySheetDialog
wxQueryLayoutInfoEvent
wxRearrangeCtrl
wxRearrangeDialog
wxRearrangeList
wxRegConfig
wxRegEx
wxRegKey
wxRibbonBar
wxRibbonBarEvent
wxRibbonButtonBar
wxRibbonButtonBarEvent
wxRibbonControl
wxRibbonGallery
wxRibbonGalleryEvent
wxRibbonMSWArtProvider
wxRibbonPage
wxRibbonPanel
wxRibbonPanelEvent
wxRibbonToolBar
wxRibbonToolBarEvent
wxRichMessageDialog
wxRichTextAction
wxRichTextBox
wxRichTextBufferDataObject
wxRichTextCell
wxRichTextCharacterStyleDefinition
wxRichTextCommand
wxRichTextCompositeObject
wxRichTextCtrl
wxRichTextDrawingHandler
wxRichTextEvent
wxRichTextField
wxRichTextFieldType
wxRichTextFieldTypeStandard
wxRichTextFileHandler
wxRichTextFormattingDialog
wxRichTextHTMLHandler
wxRichTextImage
wxRichTextListStyleDefinition
wxRichTextObject
wxRichTextParagraph
wxRichTextParagraphLayoutBox
wxRichTextParagraphStyleDefinition
wxRichTextPlainText
wxRichTextPlainTextHandler
wxRichTextPrinting
wxRichTextPrintout
wxRichTextStyleComboCtrl
wxRichTextStyleDefinition
wxRichTextStyleListBox
wxRichTextStyleOrganiserDialog
wxRichTextTable
wxRichTextXMLHandler
wxSashEvent
wxSashLayoutWindow
wxSashWindow
wxSearchCtrl
wxSimplebook
wxSimpleHtmlListBox
wxSingleInstanceChecker
wxSocketClient
wxSocketEvent
wxSocketServer
wxSound
wxSpinButton
wxSpinCtrl
wxSpinCtrlDouble
wxSpinDoubleEvent
wxSplashScreen
wxSplitterEvent
wxSplitterWindow
wxStackWalker
wxStaticLine
wxStringOutputStream
wxStringProperty
wxStringTokenizer
wxStyledTextCtrl
wxStyledTextEvent
wxSVGFileDC
wxSymbolPickerDialog
wxSystemColourProperty
wxTarEntry
wxTaskBarIcon
wxTextAttrDimension
wxTextAttrDimensionConverter
wxTimePickerCtrl
wxTipWindow
wxToggleButton
wxTreebook
wxTreeCtrl
wxTreeEvent
wxTreeListCtrl
wxUIntProperty
wxURL
wxVariantDataErrorCode
wxVariantDataSafeArray
wxVListBox
wxVScrolledWindow
wxWebKitBeforeLoadEvent
wxWebKitCtrl
wxWebKitNewWindowEvent
wxWebKitStateChangedEvent
wxWizard
wxWizardEvent
wxWizardPage
wxWizardPageSimple
wxWrapSizer
wxXmlAttribute
wxXmlDocument
wxXmlNode
wxXmlResource
wxZipEntry";
    for line in blocklist.lines() {
        let mut line = line.to_owned();
        line.trim();
        if line.len() > 0 {
            class_blocklist.insert(line);
        }
    }

    for (name, refid) in get_all_classes(&index_root) {
        if !class_blocklist.contains(&name)
            && !name.contains(":")
            && !name.contains("<")
            && !name.contains(" ")
        {
            let filename = format!("{}/{}.xml", xml_path, refid);
            if let Err(e) = generate_constructor_wrappers(
                &Element::from_reader(&mut Reader::from_file(&filename)?)?,
                &mut hxx_stream,
                &mut cxx_stream,
            ) {
                println!("Error processing file {}: {:?}", filename, e);
            }
        }
    }
    Ok(())
}
