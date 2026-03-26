#![allow(dead_code)] // Generated types used by T-029 (client) and T-030 (mapping)

use graphql_client::GraphQLQuery;

type DateTime = String;
#[allow(clippy::upper_case_acronyms)]
type URI = String;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github/graphql/schema.graphql",
    query_path = "src/github/graphql/dashboard.graphql",
    response_derives = "Debug, Clone",
    variables_derives = "Debug"
)]
pub struct DashboardData;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github/graphql/schema.graphql",
    query_path = "src/github/graphql/dashboard.graphql",
    response_derives = "Debug, Clone",
    variables_derives = "Debug"
)]
pub struct RecentActivity;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github/graphql/schema.graphql",
    query_path = "src/github/graphql/pull_request_detail.graphql",
    response_derives = "Debug, Clone",
    variables_derives = "Debug"
)]
pub struct PullRequestDetail;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_query_variables_construction() {
        let vars = dashboard_data::Variables {
            review_query: "type:pr review-requested:octocat state:open".into(),
            my_prs_query: "type:pr author:octocat state:open".into(),
            issues_query: "type:issue assignee:octocat state:open".into(),
            first: 25,
        };
        assert_eq!(vars.first, 25);
        assert!(vars.review_query.contains("review-requested:octocat"));
        assert!(vars.my_prs_query.contains("author:octocat"));
        assert!(vars.issues_query.contains("assignee:octocat"));
    }

    #[test]
    fn test_activity_query_variables_construction() {
        let vars = recent_activity::Variables {
            activity_query: "type:pr type:issue involves:octocat updated:>2026-03-01".into(),
            first: 50,
        };
        assert_eq!(vars.first, 50);
        assert!(vars.activity_query.contains("involves:octocat"));
    }

    #[test]
    fn test_pull_request_detail_variables_construction() {
        let vars = pull_request_detail::Variables {
            owner: "mpiton".into(),
            name: "prism".into(),
            number: 42,
        };
        assert_eq!(vars.owner, "mpiton");
        assert_eq!(vars.name, "prism");
        assert_eq!(vars.number, 42);
    }
}
