//! What the shared shell models need to know about an app's routes. An app implements [`Routes`] on a zero-sized type
//! and names it as the type parameter of the models ([`crate::tabs::PageTabs`], [`crate::tiles::TileLayout`],
//! [`crate::workspace::Workspaces`]). The trait has no `self`, so the models stay plain data with no runtime cost.
pub trait Routes: Clone + Default + std::fmt::Debug + 'static {
    /// A rail area. Saved by its id (see [`Routes::area_id`]).
    type Area: Copy + Eq + std::fmt::Debug;
    /// The route new tabs, empty groups and new workspaces open on.
    const HOME_ROUTE: &'static str;
    /// The rail area a new workspace starts in.
    const HOME_AREA: Self::Area;
    /// The page title of `route`, or `None` when `route` is not a known page (the tab keeps its title then).
    fn title(route: &str) -> Option<String>;
    /// The saved id of `area`.
    fn area_id(area: Self::Area) -> &'static str;
    /// The area saved as `id`, or `None` for an unknown id.
    fn area_from_id(id: &str) -> Option<Self::Area>;
}

/// Routes and areas for the shell models' tests. The route titles are the ones the Space manifest gives these routes,
/// and a route that is not listed here is not a known page.
#[cfg(test)]
pub(crate) mod test_routes {
    use super::Routes;

    /// The two rail areas the model tests use.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum TestArea {
        Home,
        Projects,
    }

    /// The test routes: Home is `/home`.
    #[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
    pub struct TestRoutes;

    impl Routes for TestRoutes {
        type Area = TestArea;
        const HOME_ROUTE: &'static str = "/home";
        const HOME_AREA: TestArea = TestArea::Home;

        fn title(route: &str) -> Option<String> {
            let title = match route {
                "/home" => "Dashboard",
                "/home/todos" => "Tasks",
                "/home/notes" => "Notes",
                "/home/time" => "Timesheet",
                _ => return None,
            };
            Some(title.to_owned())
        }

        fn area_id(area: TestArea) -> &'static str {
            match area {
                TestArea::Home => "home",
                TestArea::Projects => "projects",
            }
        }

        fn area_from_id(id: &str) -> Option<TestArea> {
            [TestArea::Home, TestArea::Projects]
                .into_iter()
                .find(|&area| Self::area_id(area) == id)
        }
    }
}
