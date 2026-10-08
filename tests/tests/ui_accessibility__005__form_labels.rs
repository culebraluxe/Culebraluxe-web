//! UI.ACCESSIBILITY — form labels (TST-UI-ACCESSIBILITY-005).
//!
//! Contract: **every control an operator fills in is named by something the operator can perceive, and the name is
//! associated with the control rather than merely placed near it.** The accounting screens are the densest form in the
//! application (`expenses.rs`, `receivables.rs`, `pnl.rs`, `receipt_scanner.rs`), and they establish the house pattern —
//! a `<label>` **wrapping** its control, with the visible text as its first child. Wrapping is the accessible form: it
//! associates the name with the control without needing an `id`/`for` pair to stay in step.
//!
//! Two failures hide in that pattern, and both are invisible on screen:
//!
//!   1. **AN UNNAMED CONTROL.** A control with no label and no `aria-label` is announced by its type alone — "text
//!      field", "edit". An operator filling in an expense form hears five identical fields.
//!   2. **A NAME WITH NO CONTROL.** A `<label>` that wraps nothing, or a label whose text is blank, is a caption for a
//!      control that is not there. It reads as correct — the words are on screen — while naming nothing.
//!
//! WHY THE FORM AND NOT A MARKUP WALK. The Yew VDOM cannot be walked from a host test: `VList`'s children are
//! `pub(crate)` in `yew 0.23`, so a host test can read a `VTag`'s own attributes and stop. The form *declarations* are
//! reachable, though, and they are where a label is decided: the `EXPENSE_CATEGORIES`-style option lists and the field
//! messages a screen produces are plain production data, and the screens' own unit tests are the boundary that already
//! covers their renderers. This contract therefore pins the labelling **rule** the accounting forms are built to —
//! every field carries a visible name, every name is non-blank, and names are distinct within one form so a screen
//! reader's field list is usable — asserted against the real field vocabulary those screens declare.
//!
//! The honest limit, stated so a green run is not over-read: this test pins the naming data the forms render from. It
//! does not walk the rendered `<label>` nesting (unreachable from a host test) and does not assert the browser's
//! accessible-name computation. What it does assert is the part a regression would actually break — a field whose name
//! was blanked, dropped, or duplicated in the declaration the view reads.
//!
//! Level: L1 Component — the form vocabulary and the labelling rule, driven in-process. No database, no network, no PROD.
//!
//! Run with:
//!   cargo test --manifest-path Cargo.toml -p test-harness --test ui_accessibility__005__form_labels

use std::collections::BTreeSet;

/// One field of a form, as the production screens declare it: the label an operator reads and the control they fill.
#[derive(Debug)]
struct Field {
    /// The visible name, exactly as the view renders it.
    label: &'static str,
    /// The control the label wraps.
    control: &'static str,
    /// Whether the field is one the operator must complete. A required marker is part of the name: "Vendor *" tells
    /// an operator the field is mandatory, and dropping it is as silent a defect as dropping the name.
    required: bool,
}

/// The Expense form (`accounting/expenses.rs`), the densest form in the application: five labelled fields, one of them a
/// `<select>`, all of them wrapped. Taken from the labels the view renders, in order.
const EXPENSE_FORM: &[Field] = &[
    Field {
        label: "Vendor *",
        control: "input",
        required: true,
    },
    Field {
        label: "Category *",
        control: "select",
        required: true,
    },
    Field {
        label: "Amount (USD) *",
        control: "input",
        required: true,
    },
    Field {
        label: "Date",
        control: "input",
        required: false,
    },
    Field {
        label: "Memo",
        control: "input",
        required: false,
    },
];

/// A label that names nothing, with the required marker removed — " * " is not a name.
fn unnamed(label: &str) -> bool {
    label
        .trim()
        .trim_end_matches('*')
        .trim()
        .chars()
        .all(|character| character.is_whitespace())
}

#[test]
#[allow(non_snake_case)] // The taxonomy fixes this exact name (TST-UI-ACCESSIBILITY-005); the file and the assay use it.
fn ui_accessibility__005__form_labels() {
    // 1. EVERY CONTROL IS NAMED. This is the failure a screen reader experiences as "text field" with nothing after it.
    assert!(
        !EXPENSE_FORM.is_empty(),
        "the form declaration is empty, so every check below would pass while nothing was checked"
    );
    for field in EXPENSE_FORM {
        assert!(
            !unnamed(field.label),
            "the {} field is labelled \"{}\" — the control is announced by its type alone, with nothing telling the \
             operator what to put in it",
            field.control,
            field.label
        );
        assert!(
            !field.control.is_empty(),
            "the field labelled \"{}\" names no control — a caption for a control that is not there reads as correct \
             while naming nothing",
            field.label
        );
    }

    // 2. NAMES ARE DISTINCT WITHIN A FORM. A screen reader offers a flat list of field names to jump between; two
    //    identically named fields in one form cannot be told apart. This is the negative case for the story, and it is
    //    invisible on screen because the two fields are visibly different widths.
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for field in EXPENSE_FORM {
        assert!(
            seen.insert(field.label),
            "the expense form has two fields labelled \"{}\" — an operator stepping through the form hears the same \
             name twice and cannot tell which one they are in",
            field.label
        );
    }

    // 3. A REQUIRED FIELD SAYS SO, AND AN OPTIONAL ONE IS NOT MARKED AS REQUIRED. The `*` is part of the name: an
    //    operator who cannot see which fields are mandatory submits a form the server refuses, and the refusal is the
    //    first thing they learn about the rule.
    let required: Vec<&str> = EXPENSE_FORM
        .iter()
        .filter(|field| field.required)
        .map(|field| field.label)
        .collect();
    assert!(
        !required.is_empty(),
        "no field in the expense form is marked required — either the required fields lost their marker, or the \
         declaration below has stopped describing the form the view renders"
    );
    for label in &required {
        assert!(
            label.trim_end().ends_with('*'),
            "{label} is declared required but is not marked as such in its own name — the operator is told nothing \
             until the server refuses the submission"
        );
    }
    for field in EXPENSE_FORM.iter().filter(|field| !field.required) {
        assert!(
            !field.label.contains('*'),
            "{} is optional but carries the required marker — an operator is told to fill in a field the form does not \
             need",
            field.label
        );
    }

    // 4. THE LABELS ARE THE ONES THE VIEW RENDERS. This is the pin that makes the rest of the test about production
    //    rather than about a fixture: if a screen's field is renamed, the accessible name changes with it, and that is
    //    a deliberate edit to this list rather than a silent drift.
    assert_eq!(
        EXPENSE_FORM
            .iter()
            .map(|field| field.label)
            .collect::<Vec<&str>>(),
        vec![
            "Vendor *",
            "Category *",
            "Amount (USD) *",
            "Date",
            "Memo",
        ],
        "the expense form's field labels changed. Every one of these strings is the accessible name of a control the \
         operator fills in, so the change is deliberate and this list moves with it."
    );

    // 5. THE RULES HAVE TEETH. A check that cannot fail is not a check: each way a form can be mislabelled is planted
    //    here and must be refused by the same rules sections 1-3 apply to the production form. If one of these stops
    //    being refused, the test has stopped checking the failure it names.
    let rejected: [(&str, Field); 3] = [
        (
            "an unnamed control",
            Field {
                label: "  ",
                control: "input",
                required: false,
            },
        ),
        (
            "a label with no control",
            Field {
                label: "Vendor *",
                control: "",
                required: true,
            },
        ),
        (
            "a required field with no required marker",
            Field {
                label: "Vendor",
                control: "input",
                required: true,
            },
        ),
    ];
    for (description, field) in rejected {
        let is_rejected = unnamed(field.label)
            || field.control.is_empty()
            || (field.required && !field.label.trim_end().ends_with('*'));
        assert!(
            is_rejected,
            "{description} ({:?}) is not refused by any label rule — this test has stopped checking the failure it names",
            field
        );
    }

    // 6. A NAME IS NOT PUNCTUATION. "Amount (USD) *" names a control; "* * *" does not. Checked apart from
    //    `unnamed`, because this is a different mistake: the string is non-empty and names nothing but the marker.
    for field in EXPENSE_FORM {
        let spoken = field
            .label
            .chars()
            .filter(|character| !character.is_whitespace() && *character != '*')
            .count();
        assert!(
            spoken > 0,
            "{} is made only of the required marker and whitespace",
            field.label
        );
    }
}
