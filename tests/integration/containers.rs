use std::env;
use std::process::Command;
use std::sync::LazyLock;
use std::time::Duration;

use ctor::{ctor, dtor};
use log::LevelFilter;
use sqlx::{Pool, Postgres};
use sqlx::postgres::PgPoolOptions;
use testcontainers::{Container, GenericImage, ImageExt};
use testcontainers::core::{IntoContainerPort, Mount, WaitFor};
use testcontainers::core::logs::LogSource;
use testcontainers::core::wait::LogWaitStrategy;
use testcontainers::runners::SyncRunner;

const CONFIG_FOLDER_PATH: &str = "/tests/integration/config";
const INIT_SQL_PATH: &str = "/db/init.sql";
const CONTAINER_PATH: &str = "/docker-entrypoint-initdb.d/init.sql";


const POSTGRES_DOCKER_IMAGE: &str = "postgres";
const POSTGRES_IMAGE_TAG: &str = "17.2";
const POSTGRES_USER_VALUE: &str = "postgres";
const POSTGRES_PWD_VALUE: &str = "postgres";
const POSTGRES_DB_VALUE: &str = "postgres";

pub static POSTGRES_CONTAINER: LazyLock<(Container<GenericImage>, String, u16)> = LazyLock::new(|| {
    let current_dir = env::current_dir().expect("Get Current Dir");
    let current_dir_str = current_dir.to_str().expect("Get Current Dir as Str");

    let container = GenericImage::new(POSTGRES_DOCKER_IMAGE, POSTGRES_IMAGE_TAG)
        .with_wait_for(WaitFor::Log(LogWaitStrategy::new(LogSource::StdOut, "PostgreSQL init process complete; ready for start up.")))
        .with_wait_for(WaitFor::Duration { length: Duration::from_secs(1) })
        .with_exposed_port(5432.tcp())
        .with_env_var("POSTGRES_USER", POSTGRES_USER_VALUE)
        .with_env_var("POSTGRES_PASSWORD", POSTGRES_PWD_VALUE)
        .with_env_var("POSTGRES_DB", POSTGRES_DB_VALUE)
        .with_mount(Mount::bind_mount(format!("{}{CONFIG_FOLDER_PATH}{INIT_SQL_PATH}", current_dir_str), CONTAINER_PATH))
        .start().expect("Postgres started");


    let host = container.get_host().expect("Get Postgres Container Port Host").to_string();
    let port = container.get_host_port_ipv4(5432).expect("Get Postgres Container Port retrieved");
    (container, host, port)
});


#[ctor]
fn init() {
    let _ = &*POSTGRES_CONTAINER;
    env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .init();
}


pub async fn get_db_pool() -> Pool<Postgres> {
    let (_, host, port) = &*POSTGRES_CONTAINER;

    let conn_info = format!(
        "postgres://{}:{}@{}:{}/{}",
        POSTGRES_USER_VALUE,
        POSTGRES_PWD_VALUE,
        host,
        port,
        POSTGRES_DB_VALUE
    );
    PgPoolOptions::new()
        .max_connections(1)
        .min_connections(1)
        .connect(conn_info.as_str())
        .await.expect("Get Database pool")
}


#[dtor]
fn destroy() {
    let container_id = get_container_id(&format!("{POSTGRES_DOCKER_IMAGE}:{POSTGRES_IMAGE_TAG}"));
    stop_container(&container_id);
    remove_container(&container_id);
}


fn get_container_id(image: &str) -> String {
    let output = Command::new("docker")
        .arg("ps")
        .arg("-f")
        .arg(format!("ancestor={image}"))
        .arg("--latest")
        .arg("--quiet")
        .output()
        .expect("Failed to execute command");

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string()
}
fn stop_container(container_id: &str) {
    Command::new("docker")
        .arg("stop")
        .arg(container_id)
        .output()
        .expect("Failed to stop the container");
}

fn remove_container(container_id: &str) {
    Command::new("docker")
        .arg("rm")
        .arg(container_id)
        .output()
        .expect("Failed to remove the container");
}








