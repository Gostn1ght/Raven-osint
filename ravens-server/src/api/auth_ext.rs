// Fix: expose is_healthy_pub from server module for commands.rs
// Add to server.rs inside ravens-client:

// pub async fn is_healthy_pub() -> bool { is_healthy().await }
