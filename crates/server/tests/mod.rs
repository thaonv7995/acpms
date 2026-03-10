//! API Integration Tests
//!
//! Test suite for all API endpoints using axum-test or tower-test utilities.
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all tests
//! cargo test --package acpms-server
//!
//! # Run specific test module
//! cargo test --package acpms-server test_auth
//!
//! # Run with output
//! cargo test --package acpms-server -- --nocapture
//!
//! # Run ignored tests (require database)
//! cargo test --package acpms-server -- --ignored
//! ```
//!
//! ## Test Database Setup
//!
//! Tests require a test database. Set `DATABASE_URL` environment variable:
//!
//! ```bash
//! PGPORT=$(docker compose port postgres 5432 | awk -F: '/127\.0\.0\.1/ {print $NF; exit}')
//! export DATABASE_URL="postgresql://acpms_user:acpms_password@127.0.0.1:${PGPORT}/acpms_test"
//! ```
//!
//! Or export a DSN that points to your published PostgreSQL port.

pub mod agent_activity_tests;
pub mod auth_tests;
pub mod dashboard_tests;
pub mod health_tests;
pub mod helpers;
pub mod project_assistant_tests;
pub mod projects_tests;
pub mod requirement_breakdowns_tests;
pub mod requirements_tests;
pub mod reviews_tests;
pub mod sprints_tests;
pub mod task_attempts_tests;
pub mod tasks_tests;
pub mod users_tests;
