use crate::TimberDB;
use std::sync::Arc;
use warp::{Filter, Reply, Rejection}; // Import Reply and Rejection explicitly
use futures::FutureExt; // for boxed()

pub mod server;
pub mod handlers;
pub mod models;

pub use server::ApiServer;
pub use models::*;

/// Create the main API routes
pub fn create_routes(
    db: Arc<TimberDB>,
) -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone { // Simplified the return type to use Rejection directly
    let db_filter = warp::any().map(move || db.clone());

    // CORS headers
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type", "authorization"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE"]);

    // Health check
    let health = warp::path("health")
        .and(warp::get())
        .and_then(handlers::health_check);

    // Partition routes
    let partitions = warp::path("partitions");

    let list_partitions = partitions
        .and(warp::path::end())
        .and(warp::get())
        .and(db_filter.clone())
        .and_then(|db| handlers::list_partitions(db).boxed());

    let create_partition = partitions
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and(db_filter.clone())
        .and_then(|body, db| handlers::create_partition(body, db).boxed());

    let get_partition = partitions
        .and(warp::path::param::<String>())
        .and(warp::path::end())
        .and(warp::get())
        .and(db_filter.clone())
        .and_then(|partition_id, db| handlers::get_partition(partition_id, db).boxed());

    // Log entry routes
    let logs = warp::path("logs");

    let append_log = logs
        .and(warp::path::param::<String>()) // partition_id
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::body::json())
        .and(db_filter.clone())
        .and_then(|partition_id, body, db| handlers::append_log(partition_id, body, db).boxed());

    let get_log = logs
        .and(warp::path::param::<String>()) // partition_id
        .and(warp::path::param::<u64>())    // entry_id
        .and(warp::path::end())
        .and(warp::get())
        .and(db_filter.clone())
        .and_then(|partition_id, entry_id, db| handlers::get_log(partition_id, entry_id, db).boxed());

    // Query routes
    let query = warp::path("query")
        .and(warp::post())
        .and(warp::body::json())
        .and(db_filter.clone())
        .and_then(|body, db| handlers::query_logs(body, db).boxed());

    // Metrics route
    let metrics = warp::path("metrics")
        .and(warp::get())
        .and(db_filter.clone())
        .and_then(|db| handlers::get_metrics(db).boxed());

    // Build partition routes
    let partition_routes = list_partitions
        .or(create_partition)
        .or(get_partition)
        .boxed(); // Box this intermediate filter

    // Build log routes 
    let log_routes = append_log
        .or(get_log)
        .boxed(); // Box this intermediate filter

    // Combine all API routes
    let api_routes = partition_routes
        .or(log_routes)
        .or(query)
        .or(metrics)
        .boxed(); // Box the final combined API routes

    // API versioning
    let api_v1 = warp::path("api")
        .and(warp::path("v1"))
        .and(api_routes);

    // Combine all routes
    let routes = health.or(api_v1);

    routes
        .with(cors)
        .with(warp::trace::request())
        .recover(handlers::handle_rejection)
        .boxed() // Box the very final filter as well
}