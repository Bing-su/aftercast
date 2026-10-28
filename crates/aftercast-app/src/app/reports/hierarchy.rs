use super::*;
use crate::components::{Button, ButtonVariant};
use aftercast::HierarchyXmlDepth;
use dioxus::prelude::*;
#[derive(Clone)]
pub(super) struct Draft {
    pub depth: HierarchyXmlDepth,
}
impl Default for Draft {
    fn default() -> Self {
        Self {
            depth: HierarchyXmlDepth::MainTable,
        }
    }
}
pub(super) fn label(depth: HierarchyXmlDepth) -> &'static str {
    match depth {
        HierarchyXmlDepth::MainTable => "hierarchy-report-main",
        HierarchyXmlDepth::DamageTypeTables => "hierarchy-report-summary",
        HierarchyXmlDepth::AllTables => "hierarchy-report-all",
    }
}
impl AppState {
    pub(super) fn report_hierarchy_depth(
        &mut self,
        token: Token,
        depth: HierarchyXmlDepth,
    ) -> bool {
        if token != self.report_token()
            || self.screen != "analysis"
            || self.reports.pending.is_some()
            || !matches!(self.phase, Phase::Idle | Phase::Ready)
        {
            return false;
        }
        self.reports.hierarchy.depth = depth;
        self.reports.leave();
        true
    }
    pub(super) fn report_hierarchy_begin(
        &mut self,
        token: Token,
    ) -> Result<Option<(Token, Request)>, &'static str> {
        if token != self.report_token() || !self.report_ready() {
            return Ok(None);
        }
        let target = self.selected.ok_or("report-select")?;
        let title = self
            .encounters
            .iter()
            .find(|row| row.id == target)
            .map(|row| format!("{} / {}", row.zone, row.title))
            .ok_or("report-select")?;
        // Capture and render share this admitted request; edits cannot retarget its depth or encounter.
        Ok(Some(self.report_admit(Request::Hierarchy(
            target,
            self.reports.hierarchy.depth,
            title,
        ))))
    }
}
#[component]
pub(super) fn Editor() -> Element {
    let mut state = use_context::<Signal<AppState>>();
    let current = state.read();
    let token = current.report_token();
    let ready = current.report_ready();
    let depth = current.reports.hierarchy.depth;
    let blocked =
        current.reports.pending.is_some() || !matches!(current.phase, Phase::Idle | Phase::Ready);
    drop(current);
    rsx! {
        section {
            class: "space-y-4 border-t border-border pt-4",
            "aria-label": super::super::tr("hierarchy-report-title"),
            h3 { class: "text-sm font-semibold", {super::super::tr("hierarchy-report-title")} }
            p { class: "max-w-prose text-sm leading-7 text-muted-foreground",
                {super::super::tr("hierarchy-report-help")}
            }
            div { class: "flex flex-wrap gap-2",
                for choice in [
                    HierarchyXmlDepth::MainTable,
                    HierarchyXmlDepth::DamageTypeTables,
                    HierarchyXmlDepth::AllTables,
                ]
                {
                    Button {
                        variant: ButtonVariant::Outline,
                        disabled: blocked,
                        aria_pressed: Some(depth == choice),
                        onclick: move |_| {
                            state.write().report_hierarchy_depth(token, choice);
                        },
                        {super::super::tr(label(choice))}
                    }
                }
            }
            Button {
                disabled: !ready,
                onclick: move |_| {
                    let result = state.write().report_hierarchy_begin(token);
                    match result {
                        Ok(work) => super::view::execute(state, work),
                        Err(key) => state.write().reports.error = Some(super::super::tr(key)),
                    }
                },
                {super::super::tr("report-generate")}
            }
        }
    }
}
