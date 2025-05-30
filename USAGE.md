# TimberDB Usage Guide 📖

<div align="center">

![TimberDB Usage](https://via.placeholder.com/800x150/4A90E2/FFFFFF?text=TimberDB+Usage+Guide+-+Complete+Reference)

**Complete guide to using TimberDB for log management, analytics, and real-time insights**

[🔍 Query Reference](#query-reference) • [📊 Analytics](#analytics-and-aggregations) • [🚀 Performance](#performance-optimization) • [💡 Best Practices](#best-practices)

</div>

---

## 📋 Table of Contents

| Section | Description |
|---------|-------------|
| [Getting Started](#getting-started) | First steps with TimberDB |
| [Data Management](#data-management) | Partitions, logs, and data lifecycle |
| [Query Reference](#query-reference) | Comprehensive query documentation |
| [Advanced Filtering](#advanced-filtering) | Complex query patterns and techniques |
| [Analytics and Aggregations](#analytics-and-aggregations) | Statistical analysis and reporting |
| [Real-time Queries](#real-time-queries) | Streaming and live data analysis |
| [Performance Optimization](#performance-optimization) | Tuning for maximum performance |
| [Monitoring and Observability](#monitoring-and-observability) | Tracking system health and performance |
| [Integration Patterns](#integration-patterns) | Common integration scenarios |
| [Troubleshooting](#troubleshooting) | Common issues and solutions |
| [Best Practices](#best-practices) | Recommended approaches and patterns |
| [Advanced Topics](#advanced-topics) | Expert-level features and techniques |

---

## Getting Started

TimberDB provides a powerful yet intuitive interface for managing log data at scale. The system is designed around the concept of partitions, which serve as logical containers for related log entries. Understanding how to effectively organize, insert, and query your data forms the foundation for all advanced TimberDB operations.

### Understanding Partitions and Data Organization

Partitions in TimberDB represent logical groupings of log data that share common characteristics or access patterns. Think of partitions as separate databases within your TimberDB instance, each optimized for specific types of log data. When designing your partition strategy, consider factors such as data volume, query patterns, retention requirements, and organizational boundaries within your application or infrastructure.

A well-designed partition strategy can dramatically improve query performance and operational efficiency. For example, you might create separate partitions for different applications, environments (development, staging, production), or log levels (errors, warnings, info). This separation allows you to apply different retention policies, configure different compression settings, and optimize queries that typically operate within these boundaries.

```bash
# Create partitions for different environments and applications
curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "production-web-server-errors",
    "tags": {
      "environment": "production",
      "application": "web-server",
      "log_level": "error",
      "team": "backend",
      "criticality": "high"
    },
    "retention_days": 365
  }'

curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "development-application-logs",
    "tags": {
      "environment": "development",
      "application": "all",
      "team": "engineering"
    },
    "retention_days": 30
  }'

curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "security-audit-logs",
    "tags": {
      "category": "security",
      "compliance": "required",
      "sensitivity": "high"
    },
    "retention_days": 2555  # 7 years for compliance
  }'
```

### Basic Log Entry Management

Log entries in TimberDB consist of a timestamp, source identifier, arbitrary tags for metadata, and a message field containing the actual log content. The flexible schema allows you to include any structured metadata as tags, which can then be used for efficient filtering and analysis. Understanding how to structure your log entries effectively is crucial for building efficient queries and maintaining good performance as your data volume grows.

The timestamp field deserves special attention as it serves as the primary organizing principle for log data. TimberDB automatically indexes log entries by timestamp, making time-range queries extremely efficient. When possible, include precise timestamps with your log entries rather than relying on ingestion time, as this provides better accuracy for analysis and debugging scenarios.

```bash
# Store the partition ID from previous creation
PARTITION_ID="550e8400-e29b-41d4-a716-446655440000"

# Add structured application logs with rich metadata
curl -X POST "http://localhost:7777/api/v1/partitions/${PARTITION_ID}/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "2024-03-15T14:30:25.123Z",
    "source": "web-server-prod-01",
    "tags": {
      "level": "error",
      "component": "user-authentication",
      "user_id": "user-12345",
      "ip_address": "192.168.1.100",
      "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
      "request_id": "req-abc-123-def",
      "session_id": "session-xyz-789",
      "error_code": "AUTH_INVALID_CREDENTIALS",
      "response_time_ms": "250",
      "endpoint": "/api/v1/login"
    },
    "message": "Authentication failed for user user-12345 from IP 192.168.1.100: invalid credentials provided after 3 attempts"
  }'

# Add database performance logs
curl -X POST "http://localhost:7777/api/v1/partitions/${PARTITION_ID}/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "2024-03-15T14:30:30.456Z",
    "source": "database-cluster-primary",
    "tags": {
      "level": "warn",
      "component": "query-executor",
      "database": "user_profiles",
      "table": "user_sessions",
      "query_type": "SELECT",
      "execution_time_ms": "1500",
      "rows_examined": "45000",
      "rows_returned": "1",
      "index_used": "idx_user_session_timestamp",
      "connection_id": "conn-789"
    },
    "message": "Slow query detected: SELECT * FROM user_sessions WHERE user_id = ? AND created_at > ? executed in 1.5 seconds, examined 45000 rows"
  }'

# Add business metrics and events
curl -X POST "http://localhost:7777/api/v1/partitions/${PARTITION_ID}/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "2024-03-15T14:31:00.789Z",
    "source": "payment-processor",
    "tags": {
      "level": "info",
      "component": "payment-gateway",
      "transaction_id": "txn-456-789-012",
      "user_id": "user-12345",
      "amount_cents": "2999",
      "currency": "USD",
      "payment_method": "credit_card",
      "gateway": "stripe",
      "merchant_id": "merchant-abc-123",
      "status": "completed",
      "processing_time_ms": "850"
    },
    "message": "Payment processed successfully: $29.99 USD for user user-12345 via Stripe gateway"
  }'
```

### Viewing and Managing Your Data

Once you have log data in TimberDB, you can inspect partition metadata to understand the current state of your data, including entry counts, storage utilization, and active blocks. This information helps with capacity planning and performance optimization decisions.

```bash
# Check partition status and metadata
curl -X GET "http://localhost:7777/api/v1/partitions/${PARTITION_ID}"

# List all partitions to see your data organization
curl -X GET "http://localhost:7777/api/v1/partitions"

# Check system health and performance metrics
curl -X GET "http://localhost:7777/health"
curl -X GET "http://localhost:7777/metrics"
```

---

## Data Management

Effective data management in TimberDB involves understanding how to organize partitions for optimal performance, implement appropriate retention policies, and monitor data lifecycle processes. The system provides extensive flexibility in how data is structured and managed, allowing you to optimize for your specific access patterns and operational requirements.

### Partition Strategy and Design Patterns

Successful TimberDB deployments typically employ partition strategies that align with their query patterns and operational boundaries. The most effective approaches consider both technical factors like query performance and organizational factors like team boundaries and compliance requirements.

**Time-based Partitioning** works well for workloads where most queries operate on recent data or specific time ranges. You might create daily, weekly, or monthly partitions that allow you to efficiently query recent data while aging out older partitions according to retention policies. This approach also simplifies backup and archival operations since entire partitions can be handled as units.

**Application-based Partitioning** groups log data by application, service, or component boundaries. This approach works well in microservices architectures where different teams own different services and typically query only their own data. It also allows different retention policies and access controls to be applied based on the criticality and sensitivity of different applications.

**Environment-based Partitioning** separates production, staging, and development data into different partitions. This separation improves security by preventing accidental queries against production data during development work, and allows different retention and performance policies to be applied based on the environment's requirements.

**Hybrid Partitioning Strategies** combine multiple approaches, such as creating partitions that are organized by both application and environment (e.g., "production-web-server", "staging-database", "development-all"). This provides fine-grained control while keeping the number of partitions manageable.

```bash
# Example of a comprehensive partitioning strategy for a microservices architecture

# Production service partitions with extended retention
curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "prod-user-service-errors",
    "tags": {"env": "production", "service": "user-service", "level": "error"},
    "retention_days": 365
  }'

curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "prod-payment-service-all",
    "tags": {"env": "production", "service": "payment-service"},
    "retention_days": 730  # Extended retention for financial data
  }'

curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "prod-api-gateway-access",
    "tags": {"env": "production", "service": "api-gateway", "type": "access"},
    "retention_days": 90
  }'

# Staging environment with shorter retention
curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "staging-all-services",
    "tags": {"env": "staging"},
    "retention_days": 30
  }'

# Development environment with minimal retention
curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "dev-all-services",
    "tags": {"env": "development"},
    "retention_days": 7
  }'

# Special-purpose partitions for specific use cases
curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "security-audit-events",
    "tags": {"category": "security", "compliance": "sox"},
    "retention_days": 2555  # 7 years for compliance
  }'

curl -X POST http://localhost:7777/api/v1/partitions/create \
  -H "Content-Type: application/json" \
  -d '{
    "name": "performance-metrics",
    "tags": {"category": "metrics", "type": "performance"},
    "retention_days": 180
  }'
```

### Advanced Log Entry Patterns

Beyond basic log entries, TimberDB supports sophisticated patterns that enable rich analytics and complex queries. Understanding these patterns helps you structure your data for maximum utility and query efficiency.

**Structured Logging with Rich Metadata** involves including comprehensive contextual information in log entry tags. This approach transforms simple log messages into structured data that can support complex analytical queries. The key is to balance the richness of metadata with query performance and storage efficiency.

**Correlation and Tracing Support** enables connecting related log entries across different services and components through shared identifiers like request IDs, trace IDs, and session IDs. This pattern is essential for debugging distributed systems and understanding end-to-end request flows.

**Event-driven Logging** treats log entries as business events with specific schemas and semantics. This approach enables using TimberDB as both a logging system and an event store for business intelligence and analytics applications.

```bash
# Advanced structured logging with correlation support
curl -X POST "http://localhost:7777/api/v1/partitions/${PARTITION_ID}/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "2024-03-15T15:45:12.345Z",
    "source": "order-processing-service-pod-7",
    "tags": {
      "level": "info",
      "component": "order-processor",
      "operation": "process_order",
      "trace_id": "trace-abc-123-def-456",
      "span_id": "span-789-012-345",
      "request_id": "req-order-678901",
      "user_id": "user-54321",
      "customer_id": "cust-98765",
      "order_id": "order-24681",
      "order_total_cents": "15750",
      "payment_method": "credit_card",
      "shipping_method": "express",
      "warehouse_location": "fulfillment-center-east",
      "inventory_reserved": "true",
      "processing_duration_ms": "1250",
      "external_calls": "3",
      "cache_hits": "2",
      "cache_misses": "1",
      "database_queries": "5",
      "business_event": "order_processed",
      "success": "true"
    },
    "message": "Order order-24681 processed successfully for customer cust-98765: $157.50 total, express shipping to fulfillment-center-east, inventory reserved, payment authorized"
  }'

# Error logging with detailed debugging context
curl -X POST "http://localhost:7777/api/v1/partitions/${PARTITION_ID}/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "2024-03-15T15:46:30.123Z",
    "source": "inventory-service-pod-3",
    "tags": {
      "level": "error",
      "component": "inventory-manager",
      "operation": "reserve_inventory",
      "trace_id": "trace-abc-123-def-456",
      "span_id": "span-456-789-012",
      "request_id": "req-order-678901",
      "order_id": "order-24681",
      "product_sku": "WIDGET-BLUE-L",
      "requested_quantity": "2",
      "available_quantity": "1",
      "warehouse_id": "wh-east-01",
      "error_type": "insufficient_inventory",
      "error_code": "INV_001",
      "retry_attempt": "1",
      "max_retries": "3",
      "fallback_warehouses": "wh-central-01,wh-west-01",
      "business_impact": "order_delayed",
      "customer_notified": "false"
    },
    "message": "Failed to reserve inventory for order order-24681: requested 2 units of WIDGET-BLUE-L but only 1 available in warehouse wh-east-01. Attempting fallback to other warehouses."
  }'

# Performance and monitoring events
curl -X POST "http://localhost:7777/api/v1/partitions/${PARTITION_ID}/logs" \
  -H "Content-Type: application/json" \
  -d '{
    "timestamp": "2024-03-15T15:47:00.789Z",
    "source": "load-balancer-nginx-01",
    "tags": {
      "level": "info",
      "component": "load-balancer",
      "operation": "http_request",
      "request_id": "req-api-345678",
      "client_ip": "203.0.113.45",
      "user_agent": "MyApp/1.2.3 (iOS; Version 14.0)",
      "method": "POST",
      "path": "/api/v1/orders",
      "status_code": "201",
      "response_time_ms": "450",
      "upstream_server": "order-service-pod-7:8080",
      "upstream_response_time_ms": "425",
      "bytes_sent": "1024",
      "bytes_received": "256",
      "connection_time_ms": "5",
      "ssl_handshake_time_ms": "15",
      "gzip_compression": "true",
      "cache_status": "MISS",
      "rate_limit_status": "OK",
      "geographic_region": "us-east-1"
    },
    "message": "HTTP POST /api/v1/orders completed in 450ms: 201 Created, routed to order-service-pod-7, client 203.0.113.45"
  }'
```

---

## Query Reference

TimberDB's query system provides powerful capabilities for filtering, searching, and analyzing log data. The query language is designed to be intuitive for developers familiar with JSON and REST APIs while providing the sophisticated filtering capabilities needed for complex log analysis scenarios.

### Basic Query Structure and Syntax

Every TimberDB query follows a consistent structure that includes a filter specification, optional result limiting parameters, and execution control options. Understanding this structure is essential for building efficient queries that return the data you need while minimizing resource consumption.

The filter object serves as the heart of every query, specifying which log entries should be included in the results. Filters can operate on timestamps, source identifiers, tag values, and message content, with each filter type optimized for different access patterns and performance characteristics. The query system automatically chooses the most efficient execution strategy based on the filters you specify and the available indexes.

```bash
# Basic query structure with all available filter options
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T16:00:00Z"],
      "source": "web-server-prod-01",
      "tags": {
        "level": "error",
        "component": "user-authentication"
      },
      "message_contains": "authentication failed"
    },
    "limit": 100,
    "timeout_seconds": 30
  }'

# Query with empty filters to retrieve all recent entries
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {}
    },
    "limit": 50
  }'

# Query with minimal filters for broad searching
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {},
      "message_contains": "payment"
    },
    "limit": 200,
    "timeout_seconds": 60
  }'
```

### Time-based Filtering and Range Queries

Time-based queries form the foundation of most log analysis scenarios since log data is inherently temporal. TimberDB provides sophisticated time filtering capabilities that enable efficient retrieval of data from specific time periods, with automatic optimization for different time range patterns.

**Absolute Time Ranges** specify exact start and end timestamps for queries. This approach works well when you know the specific time period of interest, such as investigating an incident that occurred during a known time window. The system uses time-based indexes to efficiently identify relevant data blocks, dramatically reducing the amount of data that must be examined.

**Relative Time Ranges** express time periods relative to the current time, such as "last 24 hours" or "previous week." While the API currently requires absolute timestamps, you can calculate these ranges in your client applications to create dynamic time-based queries that adapt automatically as time progresses.

**Time Zone Considerations** require careful attention when working with distributed systems that span multiple time zones. TimberDB stores all timestamps in UTC and expects query time ranges to be specified in UTC as well. This approach ensures consistent behavior regardless of the client's local time zone, but requires client applications to handle time zone conversions appropriately.

```bash
# Query for specific time ranges with different granularities

# Last hour of data
START_TIME=$(date -u -d '1 hour ago' +%Y-%m-%dT%H:%M:%SZ)
END_TIME=$(date -u +%Y-%m-%dT%H:%M:%SZ)

curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"filter\": {
      \"time_range\": [\"${START_TIME}\", \"${END_TIME}\"],
      \"tags\": {}
    },
    \"limit\": 1000
  }"

# Specific incident investigation timeframe
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:30:00Z", "2024-03-15T14:45:00Z"],
      "tags": {
        "level": "error"
      }
    },
    "limit": 500
  }'

# Business hours analysis (9 AM to 5 PM UTC)
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T09:00:00Z", "2024-03-15T17:00:00Z"],
      "tags": {
        "component": "payment-processor"
      }
    },
    "limit": 2000
  }'

# Weekend activity analysis
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-16T00:00:00Z", "2024-03-17T23:59:59Z"],
      "tags": {
        "environment": "production"
      }
    },
    "limit": 5000
  }'
```

### Source-based Filtering

Source filtering enables you to focus your queries on log entries from specific systems, services, or components. This capability is essential for debugging issues in distributed systems where identifying the source of problems requires examining logs from specific components or instances.

The source field in TimberDB is a free-form string that can represent anything from specific server hostnames to service names to individual container instances. Developing consistent source naming conventions across your infrastructure improves the effectiveness of source-based queries and makes it easier to understand the origin of log entries.

**Exact Source Matching** provides precise filtering when you know the exact source identifier you want to examine. This approach works well for investigating issues on specific servers or service instances where you need to see all activity from that source.

**Pattern-based Source Filtering** would require implementing client-side filtering or using message content searches, since the current API supports exact source matching. However, you can structure your source identifiers hierarchically (e.g., "service.instance.pod") to enable systematic querying of related sources.

```bash
# Query logs from specific server instances
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "source": "web-server-prod-01",
      "tags": {}
    },
    "limit": 500
  }'

# Focus on specific service instances during incident investigation
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "source": "payment-service-pod-7",
      "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T15:00:00Z"],
      "tags": {}
    },
    "limit": 1000
  }'

# Database server analysis
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "source": "database-cluster-primary",
      "tags": {
        "level": "warn"
      }
    },
    "limit": 300
  }'

# Load balancer traffic analysis
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "source": "load-balancer-nginx-01",
      "tags": {
        "status_code": "500"
      }
    },
    "limit": 200
  }'
```

### Tag-based Filtering and Complex Conditions

Tag-based filtering provides the most flexible and powerful way to query log data in TimberDB. Tags represent structured metadata associated with each log entry, and the tag filtering system enables complex queries that can efficiently find specific types of events or conditions across your entire log dataset.

**Single Tag Filtering** enables straightforward queries that match specific tag values. This approach works well for common scenarios like finding all error-level logs, all entries from a specific component, or all entries associated with a particular user or transaction.

**Multiple Tag Filtering** creates AND conditions where log entries must match all specified tag criteria. This capability enables precise filtering that combines multiple dimensions of metadata, such as finding error-level logs from specific components within a particular time range.

**Tag Value Patterns** currently support exact matching, but you can structure tag values hierarchically or include multiple related values to enable flexible querying. For example, using dotted notation in tag values (like "component.subcomponent") or including multiple relevant identifiers in tag values.

```bash
# Single tag filtering for common scenarios

# All error-level logs across the system
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "level": "error"
      }
    },
    "limit": 1000
  }'

# All authentication-related events
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "component": "authentication"
      }
    },
    "limit": 500
  }'

# Production environment logs only
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "environment": "production"
      }
    },
    "limit": 2000
  }'

# Multiple tag filtering for precise queries

# Error-level authentication logs in production
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "level": "error",
        "component": "authentication",
        "environment": "production"
      }
    },
    "limit": 200
  }'

# Database performance issues
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "component": "database",
        "level": "warn",
        "query_type": "SELECT"
      }
    },
    "limit": 100
  }'

# Payment processing failures
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "component": "payment-processor",
        "status": "failed",
        "payment_method": "credit_card"
      }
    },
    "limit": 50
  }'

# User-specific activity tracking
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "user_id": "user-12345",
        "operation": "login"
      }
    },
    "limit": 25
  }'

# Business event analysis
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "business_event": "order_processed",
        "success": "true",
        "payment_method": "credit_card"
      }
    },
    "limit": 1000
  }'
```

### Message Content Filtering and Text Search

Message content filtering enables searching within the actual log message text, providing powerful capabilities for finding specific events, error messages, or patterns across your log data. This functionality complements tag-based filtering by allowing you to search for specific text patterns that may not be captured in structured tags.

**Substring Matching** through the `message_contains` filter finds all log entries whose message text contains the specified string. This search is case-sensitive and looks for exact substring matches, making it ideal for finding specific error messages, identifiers, or keywords within log content.

**Content Search Strategies** should consider the performance implications of text search operations. Searches for common words or short strings may examine large amounts of data, while searches for specific error codes or unique identifiers typically execute much faster. Combining message content searches with other filter criteria helps improve query performance and result relevance.

**Search Optimization** can be achieved by including relevant information in both structured tags and message content. This approach enables efficient queries using tag-based filtering while still supporting flexible text searches when needed. It also provides redundancy that helps ensure important information can be found through multiple query approaches.

```bash
# Basic text search within log messages

# Find authentication failure messages
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {},
      "message_contains": "authentication failed"
    },
    "limit": 100
  }'

# Search for specific error codes
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {},
      "message_contains": "ERROR_CODE_001"
    },
    "limit": 50
  }'

# Find database connection issues
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {},
      "message_contains": "connection timeout"
    },
    "limit": 200
  }'

# Combining text search with tag filtering for precision

# Authentication errors with specific message content
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "component": "authentication",
        "level": "error"
      },
      "message_contains": "invalid credentials"
    },
    "limit": 100
  }'

# Payment processing with specific error patterns
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "component": "payment-processor"
      },
      "message_contains": "transaction declined"
    },
    "limit": 75
  }'

# Performance issues with specific thresholds
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "level": "warn"
      },
      "message_contains": "exceeded threshold"
    },
    "limit": 150
  }'

# Security-related events with specific patterns
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "category": "security"
      },
      "message_contains": "suspicious activity"
    },
    "limit": 25
  }'

# Business logic errors with contextual information
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "component": "order-processor"
      },
      "message_contains": "inventory insufficient"
    },
    "limit": 100
  }'
```

---

## Advanced Filtering

Advanced filtering techniques enable sophisticated log analysis scenarios that go beyond basic field matching. These approaches combine multiple filter types, use strategic querying patterns, and leverage TimberDB's indexing capabilities to efficiently answer complex questions about your log data.

### Complex Multi-dimensional Queries

Multi-dimensional queries combine multiple filter criteria to answer complex questions about system behavior. These queries typically involve correlating information across different dimensions such as time, source, log level, and business context to provide comprehensive insights into system behavior and issues.

**Cross-service Analysis** enables investigating issues that span multiple services or components by crafting queries that examine related log entries across different sources. This approach is essential for debugging distributed systems where problems often involve interactions between multiple services.

**User Journey Analysis** tracks specific users or transactions across multiple system components by querying for shared identifiers like user IDs, session IDs, or request IDs. This technique provides end-to-end visibility into user experiences and helps identify where problems occur in complex workflows.

**Performance Correlation** combines performance-related tags and message content to identify patterns in system performance issues. These queries might look for correlations between slow database queries and application response times, or between high error rates and resource utilization patterns.

```bash
# Cross-service error correlation during a specific incident
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:30:00Z", "2024-03-15T14:45:00Z"],
      "tags": {
        "level": "error",
        "trace_id": "trace-abc-123-def-456"
      }
    },
    "limit": 500,
    "timeout_seconds": 60
  }'

# User session analysis across multiple touchpoints
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "user_id": "user-12345",
        "session_id": "session-xyz-789"
      }
    },
    "limit": 1000
  }'

# Payment processing pipeline analysis
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "transaction_id": "txn-456-789-012"
      }
    },
    "limit": 100
  }'

# Performance degradation investigation
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T15:00:00Z", "2024-03-15T16:00:00Z"],
      "tags": {
        "level": "warn"
      },
      "message_contains": "slow"
    },
    "limit": 300
  }'

# Database performance during high load periods
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "component": "database",
        "execution_time_ms": "1000"
      }
    },
    "limit": 200
  }'
```

### Pattern-based Analysis and Correlation

Pattern-based analysis involves identifying recurring themes, error patterns, or behavioral sequences within your log data. These techniques help identify systemic issues, understand usage patterns, and detect anomalies that might indicate problems or security concerns.

**Error Pattern Analysis** examines sequences of related errors or warning messages to understand the progression of system issues. This approach helps identify root causes by showing how initial problems cascade into subsequent failures across different system components.

**Temporal Pattern Detection** looks for patterns that occur at specific times or with specific frequencies. These might include regularly scheduled batch jobs, daily traffic patterns, or periodic maintenance activities that create predictable log patterns.

**Behavioral Sequence Analysis** tracks sequences of related events that represent business processes or user workflows. Understanding these patterns helps identify where processes break down and what conditions lead to successful or failed outcomes.

```bash
# Cascading failure analysis - find initial errors that led to widespread issues
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:25:00Z", "2024-03-15T14:35:00Z"],
      "tags": {
        "level": "error",
        "component": "database"
      }
    },
    "limit": 50
  }'

# Follow up with downstream impact analysis
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:30:00Z", "2024-03-15T14:40:00Z"],
      "tags": {
        "level": "error"
      },
      "message_contains": "database connection"
    },
    "limit": 200
  }'

# Authentication failure patterns - looking for potential security issues
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "component": "authentication",
        "level": "error"
      }
    },
    "limit": 1000
  }'

# Business process completion patterns
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "business_event": "order_processed",
        "success": "true"
      }
    },
    "limit": 500
  }'

# System startup and initialization patterns
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {},
      "message_contains": "startup complete"
    },
    "limit": 100
  }'
```

### Query Optimization Strategies

Query optimization in TimberDB involves understanding how different filter types perform and structuring queries to minimize resource consumption while maximizing result accuracy. Effective optimization requires understanding both the data distribution and the query execution model.

**Index-friendly Query Patterns** leverage TimberDB's indexing capabilities by structuring queries to use the most selective filters first. Time-based filters are typically the most efficient, followed by tag-based filters, with message content searches being the most resource-intensive.

**Result Set Management** involves using appropriate limit values and timeout settings to prevent queries from consuming excessive resources. Large result sets should be processed in batches or refined with additional filter criteria to improve performance and manageability.

**Query Composition Strategies** break complex analysis tasks into multiple simpler queries that can be combined client-side. This approach often performs better than single complex queries and provides more flexibility in how results are processed and analyzed.

```bash
# Efficient query pattern: time + tags + content (in order of selectivity)
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T15:00:00Z"],
      "tags": {
        "level": "error",
        "component": "payment-processor"
      },
      "message_contains": "timeout"
    },
    "limit": 100,
    "timeout_seconds": 30
  }'

# Batch processing for large time ranges - process one hour at a time
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T15:00:00Z"],
      "tags": {
        "level": "error"
      }
    },
    "limit": 1000,
    "timeout_seconds": 45
  }'

# Progressive refinement - start broad, then narrow down
# Step 1: Find all errors in timeframe
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T16:00:00Z"],
      "tags": {
        "level": "error"
      }
    },
    "limit": 500
  }'

# Step 2: Focus on specific component based on initial results
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:30:00Z", "2024-03-15T14:45:00Z"],
      "tags": {
        "level": "error",
        "component": "authentication"
      }
    },
    "limit": 200
  }'

# Memory-conscious queries for large datasets
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "level": "info",
        "component": "access-log"
      }
    },
    "limit": 50,
    "timeout_seconds": 15
  }'
```

---

## Analytics and Aggregations

While TimberDB's current API focuses on individual log entry retrieval, you can perform sophisticated analytics by combining query results with client-side processing. Understanding how to structure queries for analytical workloads and process results efficiently enables powerful insights into system behavior, performance trends, and business metrics.

### Statistical Analysis Patterns

Statistical analysis of log data provides insights into system behavior patterns, performance characteristics, and trending information that helps with capacity planning and optimization decisions. These analyses typically involve aggregating data across time periods, services, or other relevant dimensions.

**Volume Analysis** examines the quantity of log entries across different dimensions to understand activity levels, identify unusual patterns, and track growth trends. This type of analysis helps with capacity planning and identifying anomalous behavior that might indicate problems or security issues.

**Error Rate Analysis** tracks the proportion of error events relative to total activity to understand system reliability and identify degradation trends. This analysis requires querying both successful and failed events to calculate meaningful ratios and percentages.

**Performance Trend Analysis** examines response times, processing durations, and other performance metrics over time to identify trends and correlate performance changes with other system events or changes.

```bash
# Collect data for volume analysis - errors over time
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "level": "error"
      }
    },
    "limit": 5000,
    "timeout_seconds": 60
  }' > errors_daily.json

# Collect baseline data - all events for comparison
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {}
    },
    "limit": 10000,
    "timeout_seconds": 120
  }' > all_events_daily.json

# Component-specific performance data collection
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "component": "database"
      },
      "message_contains": "execution time"
    },
    "limit": 2000
  }' > database_performance.json

# Authentication success/failure analysis
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "component": "authentication"
      }
    },
    "limit": 3000
  }' > authentication_events.json

# Business metrics data collection
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "business_event": "order_processed"
      }
    },
    "limit": 1000
  }' > business_orders.json
```

### Client-side Aggregation Techniques

Since TimberDB returns individual log entries, aggregation logic must be implemented in client applications. Understanding effective aggregation patterns helps you build efficient analytical workflows that can process large datasets and generate meaningful insights.

**Time-based Aggregation** groups log entries by time intervals (hourly, daily, etc.) to identify patterns and trends. This approach requires parsing timestamps from query results and grouping entries into appropriate time buckets for analysis.

**Component-based Aggregation** groups log entries by source, component, or other categorical fields to understand the distribution of activity or errors across different system components. This analysis helps identify which components are most active or problematic.

**Tag-based Aggregation** uses the structured metadata in log entries to create detailed breakdowns across multiple dimensions. This approach enables sophisticated analysis that can correlate different aspects of system behavior.

```python
# Python example for processing TimberDB query results
import json
import requests
from datetime import datetime, timedelta
from collections import defaultdict, Counter

def fetch_logs(time_start, time_end, filters=None):
    """Fetch logs from TimberDB with specified filters"""
    query = {
        "filter": {
            "time_range": [time_start, time_end],
            "tags": filters or {}
        },
        "limit": 5000,
        "timeout_seconds": 60
    }
    
    response = requests.post(
        "http://localhost:7777/api/v1/query/execute",
        json=query
    )
    return response.json()

def analyze_error_trends(start_date, end_date):
    """Analyze error trends over time"""
    # Fetch error logs
    error_data = fetch_logs(start_date, end_date, {"level": "error"})
    
    # Group by hour
    hourly_errors = defaultdict(int)
    error_by_component = defaultdict(int)
    
    for entry in error_data.get("entries", []):
        timestamp = datetime.fromisoformat(entry["timestamp"].replace("Z", "+00:00"))
        hour_key = timestamp.strftime("%Y-%m-%d %H:00")
        hourly_errors[hour_key] += 1
        
        component = entry.get("tags", {}).get("component", "unknown")
        error_by_component[component] += 1
    
    return {
        "hourly_distribution": dict(hourly_errors),
        "component_distribution": dict(error_by_component),
        "total_errors": len(error_data.get("entries", []))
    }

def analyze_performance_metrics(start_date, end_date):
    """Analyze performance metrics from log data"""
    # Fetch performance-related logs
    perf_data = fetch_logs(start_date, end_date, {"component": "database"})
    
    response_times = []
    queries_by_type = defaultdict(list)
    
    for entry in perf_data.get("entries", []):
        tags = entry.get("tags", {})
        
        # Extract response time if available
        exec_time = tags.get("execution_time_ms")
        if exec_time:
            try:
                response_times.append(float(exec_time))
            except ValueError:
                pass
        
        # Group by query type
        query_type = tags.get("query_type", "unknown")
        if exec_time:
            queries_by_type[query_type].append(float(exec_time))
    
    # Calculate statistics
    stats = {}
    if response_times:
        response_times.sort()
        stats["avg_response_time"] = sum(response_times) / len(response_times)
        stats["median_response_time"] = response_times[len(response_times) // 2]
        stats["p95_response_time"] = response_times[int(len(response_times) * 0.95)]
        stats["p99_response_time"] = response_times[int(len(response_times) * 0.99)]
    
    return {
        "overall_stats": stats,
        "by_query_type": {
            qtype: {
                "count": len(times),
                "avg": sum(times) / len(times) if times else 0,
                "max": max(times) if times else 0
            }
            for qtype, times in queries_by_type.items()
        }
    }

def analyze_user_behavior(start_date, end_date, user_id):
    """Analyze specific user behavior patterns"""
    # Fetch user-specific logs
    user_data = fetch_logs(start_date, end_date, {"user_id": user_id})
    
    actions = []
    sessions = defaultdict(list)
    
    for entry in user_data.get("entries", []):
        timestamp = datetime.fromisoformat(entry["timestamp"].replace("Z", "+00:00"))
        tags = entry.get("tags", {})
        
        action = {
            "timestamp": timestamp,
            "operation": tags.get("operation", "unknown"),
            "component": tags.get("component", "unknown"),
            "success": tags.get("success", "unknown")
        }
        actions.append(action)
        
        session_id = tags.get("session_id")
        if session_id:
            sessions[session_id].append(action)
    
    # Analyze session patterns
    session_stats = {}
    for session_id, session_actions in sessions.items():
        session_actions.sort(key=lambda x: x["timestamp"])
        duration = session_actions[-1]["timestamp"] - session_actions[0]["timestamp"]
        
        session_stats[session_id] = {
            "duration_minutes": duration.total_seconds() / 60,
            "action_count": len(session_actions),
            "operations": Counter([a["operation"] for a in session_actions])
        }
    
    return {
        "total_actions": len(actions),
        "unique_sessions": len(sessions),
        "session_details": session_stats,
        "operation_summary": Counter([a["operation"] for a in actions])
    }

# Example usage
if __name__ == "__main__":
    # Define analysis time range
    end_time = datetime.utcnow()
    start_time = end_time - timedelta(days=1)
    
    start_str = start_time.strftime("%Y-%m-%dT%H:%M:%SZ")
    end_str = end_time.strftime("%Y-%m-%dT%H:%M:%SZ")
    
    # Perform various analyses
    print("Analyzing error trends...")
    error_analysis = analyze_error_trends(start_str, end_str)
    print(f"Total errors: {error_analysis['total_errors']}")
    print(f"Top error components: {error_analysis['component_distribution']}")
    
    print("\nAnalyzing performance metrics...")
    perf_analysis = analyze_performance_metrics(start_str, end_str)
    print(f"Performance stats: {perf_analysis['overall_stats']}")
    
    print("\nAnalyzing user behavior...")
    user_analysis = analyze_user_behavior(start_str, end_str, "user-12345")
    print(f"User activity: {user_analysis['operation_summary']}")
```

### Business Intelligence and Reporting

Log data often contains valuable business intelligence that can inform decision-making and provide insights into customer behavior, system utilization, and operational efficiency. Extracting this intelligence requires understanding how to identify business events in log data and aggregate them meaningfully.

**Revenue and Transaction Analysis** examines payment processing logs, order completion events, and other business-critical events to understand revenue patterns, transaction success rates, and customer behavior trends.

**Customer Journey Analysis** tracks user interactions across multiple system touchpoints to understand how customers use your applications and where they encounter problems or friction in their experience.

**Operational Efficiency Metrics** analyze system performance from a business perspective, looking at metrics like order processing times, customer service response times, and other operationally relevant measurements.

```bash
# Collect business transaction data for revenue analysis
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "business_event": "order_processed",
        "status": "completed"
      }
    },
    "limit": 5000,
    "timeout_seconds": 90
  }' > completed_orders.json

# Collect payment processing events for financial analysis
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "component": "payment-processor"
      }
    },
    "limit": 3000
  }' > payment_events.json

# Customer service interaction data
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "component": "customer-service"
      }
    },
    "limit": 1000
  }' > customer_service.json

# Website and API usage patterns
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "component": "api-gateway"
      }
    },
    "limit": 10000,
    "timeout_seconds": 120
  }' > api_usage.json
```

This comprehensive query reference and analytics guide provides the foundation for sophisticated log analysis workflows. The combination of TimberDB's efficient query capabilities with client-side aggregation and analysis enables powerful insights into system behavior, business performance, and operational efficiency. The next sections will cover real-time monitoring, performance optimization, and advanced operational patterns that build upon these fundamental query and analysis techniques.

---

## Real-time Queries

Real-time log analysis enables immediate detection of issues, live monitoring of system health, and rapid response to critical events. While TimberDB's current API provides point-in-time querying capabilities, you can build real-time monitoring systems by combining efficient querying patterns with client-side streaming logic and intelligent polling strategies.

### Live Monitoring Patterns

Live monitoring requires balancing the need for current information with system performance and resource utilization. Effective real-time monitoring systems use strategic querying patterns that minimize load while ensuring that critical events are detected quickly.

**Sliding Window Monitoring** implements real-time awareness by continuously querying recent time windows and processing new entries as they arrive. This approach provides near real-time visibility into system behavior while managing query load through efficient time-based filtering.

**Incremental Processing** tracks the most recent timestamp from previous queries to ensure that subsequent queries only retrieve new data. This pattern minimizes duplicate processing and reduces query overhead while maintaining comprehensive coverage of incoming log data.

**Priority-based Monitoring** focuses monitoring efforts on the most critical events and components by using targeted queries that examine high-priority log levels or specific system components that require immediate attention when issues occur.

```bash
# Real-time error monitoring with sliding window approach
# Query recent errors (last 5 minutes) every 30 seconds

CURRENT_TIME=$(date -u +%Y-%m-%dT%H:%M:%SZ)
START_TIME=$(date -u -d '5 minutes ago' +%Y-%m-%dT%H:%M:%SZ)

curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"filter\": {
      \"time_range\": [\"${START_TIME}\", \"${CURRENT_TIME}\"],
      \"tags\": {
        \"level\": \"error\"
      }
    },
    \"limit\": 500,
    \"timeout_seconds\": 10
  }" > current_errors.json

# Critical system component monitoring
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"filter\": {
      \"time_range\": [\"${START_TIME}\", \"${CURRENT_TIME}\"],
      \"tags\": {
        \"component\": \"payment-processor\",
        \"level\": \"error\"
      }
    },
    \"limit\": 100,
    \"timeout_seconds\": 5
  }" > payment_issues.json

# Authentication failure monitoring for security
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"filter\": {
      \"time_range\": [\"${START_TIME}\", \"${CURRENT_TIME}\"],
      \"tags\": {
        \"component\": \"authentication\",
        \"level\": \"error\"
      }
    },
    \"limit\": 200,
    \"timeout_seconds\": 5
  }" > auth_failures.json

# Database performance monitoring
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"filter\": {
      \"time_range\": [\"${START_TIME}\", \"${CURRENT_TIME}\"],
      \"tags\": {
        \"component\": \"database\",
        \"level\": \"warn\"
      }
    },
    \"limit\": 150
  }" > database_warnings.json
```

### Streaming Data Processing

Building effective streaming data processing on top of TimberDB requires implementing client-side logic that can efficiently process continuous streams of log data while maintaining state and detecting patterns across multiple queries.

**State Management** for streaming applications involves tracking processing checkpoints, maintaining running statistics, and correlating events across multiple query cycles. This requires careful design of data structures that can efficiently update as new data arrives.

**Event Correlation** across time windows enables detection of patterns that span multiple query intervals. This might include tracking authentication failure rates, monitoring error cascades, or detecting performance degradation trends that develop over several minutes or hours.

**Alert Generation** logic processes streaming data to identify conditions that require immediate attention. Effective alerting systems avoid false positives while ensuring that genuine issues are detected quickly and escalated appropriately.

```python
#!/usr/bin/env python3
"""
Real-time log monitoring and alerting system for TimberDB
Demonstrates streaming patterns and real-time analysis
"""

import json
import time
import requests
from datetime import datetime, timedelta
from collections import defaultdict, deque
from typing import Dict, List, Optional, Set

class TimberDBMonitor:
    def __init__(self, base_url: str = "http://localhost:7777"):
        self.base_url = base_url
        self.last_processed = {}  # Track last processed timestamp per query type
        self.error_counts = defaultdict(int)  # Running error counts
        self.performance_window = deque(maxlen=100)  # Recent performance metrics
        self.alert_cooldowns = {}  # Prevent alert spam
        
    def query_recent_logs(self, filters: Dict, window_minutes: int = 5, 
                         limit: int = 1000) -> Dict:
        """Query recent logs within specified time window"""
        end_time = datetime.utcnow()
        start_time = end_time - timedelta(minutes=window_minutes)
        
        query = {
            "filter": {
                "time_range": [
                    start_time.strftime("%Y-%m-%dT%H:%M:%SZ"),
                    end_time.strftime("%Y-%m-%dT%H:%M:%SZ")
                ],
                "tags": filters
            },
            "limit": limit,
            "timeout_seconds": 10
        }
        
        try:
            response = requests.post(
                f"{self.base_url}/api/v1/query/execute",
                json=query,
                timeout=15
            )
            response.raise_for_status()
            return response.json()
        except Exception as e:
            print(f"Query failed: {e}")
            return {"entries": [], "matched_entries": 0}
    
    def process_error_events(self) -> List[Dict]:
        """Monitor and process error events"""
        errors = self.query_recent_logs({"level": "error"}, window_minutes=2)
        alerts = []
        
        # Track error rates by component
        component_errors = defaultdict(int)
        recent_errors = []
        
        for entry in errors.get("entries", []):
            component = entry.get("tags", {}).get("component", "unknown")
            component_errors[component] += 1
            recent_errors.append({
                "timestamp": entry["timestamp"],
                "component": component,
                "source": entry.get("source", "unknown"),
                "message": entry.get("message", "")
            })
        
        # Generate alerts for high error rates
        for component, count in component_errors.items():
            if count >= 5:  # 5+ errors in 2 minutes
                alert_key = f"high_error_rate_{component}"
                if self._should_alert(alert_key, cooldown_minutes=5):
                    alerts.append({
                        "type": "high_error_rate",
                        "component": component,
                        "count": count,
                        "timeframe": "2 minutes",
                        "severity": "warning",
                        "recent_errors": recent_errors[-3:]  # Include recent examples
                    })
        
        return alerts
    
    def process_performance_events(self) -> List[Dict]:
        """Monitor system performance indicators"""
        perf_logs = self.query_recent_logs({
            "component": "database",
            "level": "warn"
        }, window_minutes=3)
        
        alerts = []
        slow_queries = []
        
        for entry in perf_logs.get("entries", []):
            tags = entry.get("tags", {})
            exec_time = tags.get("execution_time_ms")
            
            if exec_time:
                try:
                    exec_time_val = float(exec_time)
                    if exec_time_val > 1000:  # Slow query threshold
                        slow_queries.append({
                            "timestamp": entry["timestamp"],
                            "execution_time": exec_time_val,
                            "query_type": tags.get("query_type", "unknown"),
                            "database": tags.get("database", "unknown")
                        })
                        
                        self.performance_window.append(exec_time_val)
                except ValueError:
                    pass
        
        # Alert on sustained slow performance
        if len(slow_queries) >= 3:
            if self._should_alert("slow_database_performance", cooldown_minutes=10):
                avg_time = sum(q["execution_time"] for q in slow_queries) / len(slow_queries)
                alerts.append({
                    "type": "slow_database_performance",
                    "slow_query_count": len(slow_queries),
                    "average_execution_time": avg_time,
                    "timeframe": "3 minutes",
                    "severity": "warning",
                    "examples": slow_queries[:3]
                })
        
        return alerts
    
    def process_security_events(self) -> List[Dict]:
        """Monitor for potential security issues"""
        auth_failures = self.query_recent_logs({
            "component": "authentication",
            "level": "error"
        }, window_minutes=5)
        
        alerts = []
        failure_by_ip = defaultdict(int)
        failure_by_user = defaultdict(int)
        
        for entry in auth_failures.get("entries", []):
            tags = entry.get("tags", {})
            ip_address = tags.get("ip_address")
            user_id = tags.get("user_id")
            
            if ip_address:
                failure_by_ip[ip_address] += 1
            if user_id:
                failure_by_user[user_id] += 1
        
        # Alert on potential brute force attacks
        for ip, count in failure_by_ip.items():
            if count >= 10:  # 10+ failures from single IP in 5 minutes
                alert_key = f"brute_force_{ip}"
                if self._should_alert(alert_key, cooldown_minutes=15):
                    alerts.append({
                        "type": "potential_brute_force",
                        "ip_address": ip,
                        "failure_count": count,
                        "timeframe": "5 minutes",
                        "severity": "critical"
                    })
        
        # Alert on account compromise attempts
        for user, count in failure_by_user.items():
            if count >= 5:  # 5+ failures for single user
                alert_key = f"account_compromise_{user}"
                if self._should_alert(alert_key, cooldown_minutes=10):
                    alerts.append({
                        "type": "potential_account_compromise",
                        "user_id": user,
                        "failure_count": count,
                        "timeframe": "5 minutes",
                        "severity": "high"
                    })
        
        return alerts
    
    def process_business_events(self) -> List[Dict]:
        """Monitor business-critical events and metrics"""
        payment_failures = self.query_recent_logs({
            "component": "payment-processor",
            "status": "failed"
        }, window_minutes=10)
        
        alerts = []
        failure_count = len(payment_failures.get("entries", []))
        
        # Alert on high payment failure rate
        if failure_count >= 5:  # 5+ payment failures in 10 minutes
            if self._should_alert("high_payment_failures", cooldown_minutes=10):
                failure_reasons = defaultdict(int)
                for entry in payment_failures.get("entries", []):
                    tags = entry.get("tags", {})
                    error_code = tags.get("error_code", "unknown")
                    failure_reasons[error_code] += 1
                
                alerts.append({
                    "type": "high_payment_failure_rate",
                    "failure_count": failure_count,
                    "timeframe": "10 minutes",
                    "severity": "critical",
                    "failure_breakdown": dict(failure_reasons)
                })
        
        return alerts
    
    def _should_alert(self, alert_key: str, cooldown_minutes: int) -> bool:
        """Check if enough time has passed since last alert of this type"""
        now = datetime.utcnow()
        last_alert = self.alert_cooldowns.get(alert_key)
        
        if not last_alert:
            self.alert_cooldowns[alert_key] = now
            return True
        
        if now - last_alert > timedelta(minutes=cooldown_minutes):
            self.alert_cooldowns[alert_key] = now
            return True
        
        return False
    
    def send_alert(self, alert: Dict):
        """Send alert notification (customize for your notification system)"""
        severity_emoji = {
            "info": "ℹ️",
            "warning": "⚠️",
            "high": "🚨",
            "critical": "🔥"
        }
        
        emoji = severity_emoji.get(alert.get("severity", "info"), "📢")
        alert_time = datetime.utcnow().strftime("%Y-%m-%d %H:%M:%S UTC")
        
        print(f"\n{emoji} ALERT [{alert['severity'].upper()}] - {alert_time}")
        print(f"Type: {alert['type']}")
        
        if alert['type'] == 'high_error_rate':
            print(f"Component: {alert['component']}")
            print(f"Error count: {alert['count']} in {alert['timeframe']}")
            
        elif alert['type'] == 'slow_database_performance':
            print(f"Slow queries: {alert['slow_query_count']}")
            print(f"Average execution time: {alert['average_execution_time']:.2f}ms")
            
        elif alert['type'] == 'potential_brute_force':
            print(f"IP Address: {alert['ip_address']}")
            print(f"Failed attempts: {alert['failure_count']} in {alert['timeframe']}")
            
        elif alert['type'] == 'high_payment_failure_rate':
            print(f"Payment failures: {alert['failure_count']} in {alert['timeframe']}")
            print(f"Failure breakdown: {alert['failure_breakdown']}")
        
        print("-" * 50)
    
    def run_monitoring_cycle(self):
        """Execute one complete monitoring cycle"""
        print(f"Running monitoring cycle at {datetime.utcnow()}")
        
        all_alerts = []
        
        # Process different types of events
        all_alerts.extend(self.process_error_events())
        all_alerts.extend(self.process_performance_events())
        all_alerts.extend(self.process_security_events())
        all_alerts.extend(self.process_business_events())
        
        # Send any alerts
        for alert in all_alerts:
            self.send_alert(alert)
        
        if not all_alerts:
            print("No alerts generated - system appears healthy")
        
        return all_alerts
    
    def start_monitoring(self, interval_seconds: int = 30):
        """Start continuous monitoring with specified interval"""
        print(f"Starting TimberDB monitoring (interval: {interval_seconds}s)")
        print("Press Ctrl+C to stop")
        
        try:
            while True:
                self.run_monitoring_cycle()
                time.sleep(interval_seconds)
        except KeyboardInterrupt:
            print("\nMonitoring stopped by user")
        except Exception as e:
            print(f"Monitoring error: {e}")

# Example usage
if __name__ == "__main__":
    monitor = TimberDBMonitor()
    
    # Run a single monitoring cycle
    alerts = monitor.run_monitoring_cycle()
    
    # Or start continuous monitoring
    # monitor.start_monitoring(interval_seconds=30)
```

### Dashboard and Visualization Integration

Real-time monitoring systems often feed into dashboards and visualization tools that provide operators with immediate visibility into system health and performance. Understanding how to structure TimberDB queries for dashboard consumption enables effective operational monitoring.

**Dashboard Data Preparation** involves formatting query results in ways that dashboard tools can easily consume. This typically means aggregating data into time series, calculating key performance indicators, and structuring responses for efficient visualization updates.

**Real-time Metrics Calculation** computes operational metrics from raw log data, such as error rates, response time percentiles, and throughput measurements. These calculations often require maintaining running averages and statistical summaries that update as new data arrives.

**Historical Context Integration** combines real-time data with historical baselines to provide context for current system behavior. This helps operators understand whether current conditions represent normal variation or genuine anomalies that require attention.

```bash
# Generate dashboard-friendly data exports for visualization tools

# Error rate trending data (hourly buckets for last 24 hours)
for hour in {0..23}; do
    START_HOUR=$(date -u -d "${hour} hours ago" +%Y-%m-%dT%H:00:00Z)
    END_HOUR=$(date -u -d "${hour} hours ago + 1 hour" +%Y-%m-%dT%H:00:00Z)
    
    curl -X POST http://localhost:7777/api/v1/query/execute \
      -H "Content-Type: application/json" \
      -d "{
        \"filter\": {
          \"time_range\": [\"${START_HOUR}\", \"${END_HOUR}\"],
          \"tags\": {\"level\": \"error\"}
        },
        \"limit\": 5000
      }" > "errors_hour_${hour}.json"
done

# Component performance summary
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {},
      "message_contains": "response_time"
    },
    "limit": 10000,
    "timeout_seconds": 60
  }' > performance_summary.json

# Business metrics for executive dashboard
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T00:00:00Z", "2024-03-15T23:59:59Z"],
      "tags": {
        "business_event": "order_processed"
      }
    },
    "limit": 5000
  }' > business_metrics.json
```

This real-time monitoring foundation enables sophisticated operational awareness and rapid response to system issues. The combination of efficient querying patterns, intelligent alerting logic, and dashboard integration provides comprehensive visibility into system behavior while managing resource consumption effectively. The next sections will cover performance optimization techniques and best practices that help these monitoring systems scale to enterprise environments.

---

## Performance Optimization

Optimizing TimberDB performance requires understanding how different query patterns interact with the storage engine, indexing system, and cluster architecture. Effective optimization involves both client-side query design and server-side configuration tuning to achieve optimal throughput and response times for your specific workload characteristics.

### Query Performance Patterns

Understanding how TimberDB processes different types of queries enables you to structure your requests for maximum efficiency. The query execution system uses multiple optimization strategies, and knowing how these work helps you write queries that take advantage of available optimizations.

**Index Utilization Strategy** involves structuring queries to maximize the use of TimberDB's indexing capabilities. Time-based indexes are typically the most efficient, so queries that include specific time ranges will generally perform better than those that scan entire datasets. Tag-based indexes provide efficient filtering on structured metadata, while full-text searches on message content are the most resource-intensive but still optimized through specialized indexing.

**Filter Ordering and Selectivity** affects query performance significantly. The most selective filters should be specified first in your query logic, even though TimberDB's query optimizer will attempt to reorder operations automatically. Time-based filters are usually most selective, followed by specific tag values, with broad text searches being least selective.

**Result Set Management** involves using appropriate limit values and timeout settings to prevent queries from consuming excessive resources. Large result sets should be processed in batches using time-based pagination or other chunking strategies that allow efficient incremental processing.

```bash
# Highly optimized query pattern: time + specific tags + targeted text search
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T14:30:00Z"],
      "tags": {
        "level": "error",
        "component": "payment-processor",
        "error_code": "TIMEOUT"
      },
      "message_contains": "transaction_id"
    },
    "limit": 100,
    "timeout_seconds": 15
  }'

# Efficient batch processing for large time ranges
# Process one hour at a time instead of entire day
BATCH_START="2024-03-15T14:00:00Z"
BATCH_END="2024-03-15T15:00:00Z"

curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d "{
    \"filter\": {
      \"time_range\": [\"${BATCH_START}\", \"${BATCH_END}\"],
      \"tags\": {
        \"level\": \"error\"
      }
    },
    \"limit\": 1000,
    \"timeout_seconds\": 30
  }"

# Optimized tag-based filtering using most selective criteria first
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "transaction_id": "txn-specific-12345",
        "component": "payment-processor"
      }
    },
    "limit": 50,
    "timeout_seconds": 10
  }'

# Avoid resource-intensive broad searches when possible
# Instead of this expensive query:
# "message_contains": "error"
# Use this more targeted approach:
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T16:00:00Z"],
      "tags": {
        "level": "error"
      },
      "message_contains": "specific_error_code_123"
    },
    "limit": 200
  }'
```

### Client-side Optimization Techniques

Client applications can significantly improve overall system performance through intelligent query patterns, result caching, and efficient data processing. Understanding these techniques helps build applications that scale effectively as data volumes and query loads increase.

**Connection Management and Pooling** involves reusing HTTP connections when possible and implementing appropriate timeout and retry logic. While TimberDB's HTTP API is stateless, connection pooling reduces the overhead of connection establishment and provides more predictable performance characteristics.

**Query Result Caching** on the client side can dramatically reduce load on TimberDB for repeated or similar queries. Implementing intelligent caching strategies that consider data freshness requirements and query patterns helps improve application responsiveness while reducing server load.

**Parallel Query Execution** enables client applications to issue multiple concurrent queries for different time ranges or filter criteria. This approach can improve overall throughput when analyzing large datasets, but should be balanced against server capacity and resource consumption considerations.

**Progressive Data Loading** implements user interface patterns that load data incrementally as needed, rather than attempting to load large datasets all at once. This approach improves perceived performance and reduces resource consumption for interactive applications.

```python
#!/usr/bin/env python3
"""
Client-side optimization examples for TimberDB queries
Demonstrates connection pooling, caching, and parallel processing
"""

import asyncio
import aiohttp
import json
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Tuple
import hashlib
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

class OptimizedTimberDBClient:
    def __init__(self, base_url: str = "http://localhost:7777", 
                 max_connections: int = 10):
        self.base_url = base_url
        self.max_connections = max_connections
        self.query_cache = {}  # Simple in-memory cache
        self.cache_ttl = 300  # 5 minutes cache TTL
        self.connection_pool = None
        
    async def __aenter__(self):
        """Async context manager entry"""
        connector = aiohttp.TCPConnector(limit=self.max_connections)
        timeout = aiohttp.ClientTimeout(total=60)
        self.connection_pool = aiohttp.ClientSession(
            connector=connector, 
            timeout=timeout
        )
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        """Async context manager exit"""
        if self.connection_pool:
            await self.connection_pool.close()
    
    def _cache_key(self, query: Dict) -> str:
        """Generate cache key for query"""
        query_str = json.dumps(query, sort_keys=True)
        return hashlib.md5(query_str.encode()).hexdigest()
    
    def _is_cache_valid(self, timestamp: float) -> bool:
        """Check if cached result is still valid"""
        return time.time() - timestamp < self.cache_ttl
    
    async def query_with_cache(self, query: Dict) -> Dict:
        """Execute query with caching support"""
        cache_key = self._cache_key(query)
        
        # Check cache first
        if cache_key in self.query_cache:
            cached_result, timestamp = self.query_cache[cache_key]
            if self._is_cache_valid(timestamp):
                print(f"Cache hit for query (key: {cache_key[:8]}...)")
                return cached_result
        
        # Execute query
        result = await self._execute_query(query)
        
        # Cache result
        self.query_cache[cache_key] = (result, time.time())
        print(f"Query executed and cached (key: {cache_key[:8]}...)")
        
        return result
    
    async def _execute_query(self, query: Dict) -> Dict:
        """Execute single query against TimberDB"""
        if not self.connection_pool:
            raise RuntimeError("Client not properly initialized")
        
        try:
            async with self.connection_pool.post(
                f"{self.base_url}/api/v1/query/execute",
                json=query
            ) as response:
                response.raise_for_status()
                return await response.json()
        except Exception as e:
            print(f"Query failed: {e}")
            return {"entries": [], "matched_entries": 0, "error": str(e)}
    
    async def parallel_time_range_query(self, base_filter: Dict, 
                                      start_time: datetime, 
                                      end_time: datetime,
                                      chunk_hours: int = 1) -> List[Dict]:
        """Execute parallel queries across time range chunks"""
        tasks = []
        current_time = start_time
        
        while current_time < end_time:
            chunk_end = min(current_time + timedelta(hours=chunk_hours), end_time)
            
            query = {
                "filter": {
                    **base_filter,
                    "time_range": [
                        current_time.strftime("%Y-%m-%dT%H:%M:%SZ"),
                        chunk_end.strftime("%Y-%m-%dT%H:%M:%SZ")
                    ]
                },
                "limit": 1000,
                "timeout_seconds": 30
            }
            
            tasks.append(self.query_with_cache(query))
            current_time = chunk_end
        
        # Execute all queries in parallel
        results = await asyncio.gather(*tasks, return_exceptions=True)
        
        # Filter out exceptions and return successful results
        successful_results = []
        for result in results:
            if isinstance(result, Exception):
                print(f"Chunk query failed: {result}")
            else:
                successful_results.append(result)
        
        return successful_results
    
    async def optimized_error_analysis(self, 
                                     start_time: datetime, 
                                     end_time: datetime) -> Dict:
        """Perform optimized error analysis across time range"""
        
        # Define different error analysis queries
        error_queries = {
            "all_errors": {"tags": {"level": "error"}},
            "auth_errors": {"tags": {"level": "error", "component": "authentication"}},
            "payment_errors": {"tags": {"level": "error", "component": "payment-processor"}},
            "database_errors": {"tags": {"level": "error", "component": "database"}},
        }
        
        # Execute all queries in parallel
        tasks = []
        for analysis_type, filters in error_queries.items():
            query = {
                "filter": {
                    **filters,
                    "time_range": [
                        start_time.strftime("%Y-%m-%dT%H:%M:%SZ"),
                        end_time.strftime("%Y-%m-%dT%H:%M:%SZ")
                    ]
                },
                "limit": 2000,
                "timeout_seconds": 45
            }
            tasks.append((analysis_type, self.query_with_cache(query)))
        
        # Collect results
        analysis_results = {}
        for analysis_type, task in tasks:
            try:
                result = await task
                analysis_results[analysis_type] = result
            except Exception as e:
                print(f"Error analysis '{analysis_type}' failed: {e}")
                analysis_results[analysis_type] = {"entries": [], "error": str(e)}
        
        return analysis_results
    
    def clear_cache(self):
        """Clear query result cache"""
        self.query_cache.clear()
        print("Query cache cleared")
    
    def cache_stats(self) -> Dict:
        """Get cache statistics"""
        valid_entries = sum(
            1 for _, timestamp in self.query_cache.values()
            if self._is_cache_valid(timestamp)
        )
        
        return {
            "total_entries": len(self.query_cache),
            "valid_entries": valid_entries,
            "cache_hit_ratio": valid_entries / max(len(self.query_cache), 1)
        }

# Synchronous client for thread-based parallel processing
class ThreadedTimberDBClient:
    def __init__(self, base_url: str = "http://localhost:7777"):
        self.base_url = base_url
        self.query_cache = {}
        self.cache_ttl = 300
    
    def execute_query(self, query: Dict) -> Dict:
        """Execute single query (synchronous)"""
        import requests
        
        try:
            response = requests.post(
                f"{self.base_url}/api/v1/query/execute",
                json=query,
                timeout=60
            )
            response.raise_for_status()
            return response.json()
        except Exception as e:
            return {"entries": [], "matched_entries": 0, "error": str(e)}
    
    def parallel_component_analysis(self, 
                                  components: List[str], 
                                  time_range: Tuple[str, str],
                                  max_workers: int = 5) -> Dict:
        """Analyze multiple components in parallel using threads"""
        
        def analyze_component(component: str) -> Tuple[str, Dict]:
            query = {
                "filter": {
                    "time_range": list(time_range),
                    "tags": {"component": component}
                },
                "limit": 1000,
                "timeout_seconds": 30
            }
            result = self.execute_query(query)
            return component, result
        
        results = {}
        
        # Use ThreadPoolExecutor for parallel execution
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            # Submit all tasks
            future_to_component = {
                executor.submit(analyze_component, component): component 
                for component in components
            }
            
            # Collect results as they complete
            for future in as_completed(future_to_component):
                component = future_to_component[future]
                try:
                    component_name, result = future.result()
                    results[component_name] = result
                    print(f"Completed analysis for component: {component_name}")
                except Exception as e:
                    print(f"Component analysis failed for {component}: {e}")
                    results[component] = {"entries": [], "error": str(e)}
        
        return results

# Example usage and performance testing
async def performance_demo():
    """Demonstrate optimized query patterns"""
    
    async with OptimizedTimberDBClient() as client:
        print("=== TimberDB Client Performance Demo ===\n")
        
        # Test 1: Cached vs uncached queries
        print("1. Testing query caching...")
        
        query = {
            "filter": {
                "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T15:00:00Z"],
                "tags": {"level": "error"}
            },
            "limit": 500
        }
        
        # First execution (cache miss)
        start_time = time.time()
        result1 = await client.query_with_cache(query)
        first_duration = time.time() - start_time
        
        # Second execution (cache hit)
        start_time = time.time()
        result2 = await client.query_with_cache(query)
        second_duration = time.time() - start_time
        
        print(f"First query (cache miss): {first_duration:.3f}s")
        print(f"Second query (cache hit): {second_duration:.3f}s")
        print(f"Cache speedup: {first_duration/second_duration:.1f}x\n")
        
        # Test 2: Parallel time range processing
        print("2. Testing parallel time range queries...")
        
        start_time = time.time()
        parallel_results = await client.parallel_time_range_query(
            base_filter={"tags": {"level": "error"}},
            start_time=datetime(2024, 3, 15, 12, 0, 0),
            end_time=datetime(2024, 3, 15, 18, 0, 0),
            chunk_hours=1
        )
        parallel_duration = time.time() - start_time
        
        total_entries = sum(len(r.get("entries", [])) for r in parallel_results)
        print(f"Parallel processing: {parallel_duration:.3f}s")
        print(f"Total entries processed: {total_entries}")
        print(f"Queries executed: {len(parallel_results)}\n")
        
        # Test 3: Multi-dimensional analysis
        print("3. Testing optimized error analysis...")
        
        start_time = time.time()
        error_analysis = await client.optimized_error_analysis(
            start_time=datetime(2024, 3, 15, 14, 0, 0),
            end_time=datetime(2024, 3, 15, 16, 0, 0)
        )
        analysis_duration = time.time() - start_time
        
        print(f"Multi-dimensional analysis: {analysis_duration:.3f}s")
        for analysis_type, result in error_analysis.items():
            entry_count = len(result.get("entries", []))
            print(f"  {analysis_type}: {entry_count} entries")
        
        # Cache statistics
        print(f"\nCache statistics: {client.cache_stats()}")

def threaded_performance_demo():
    """Demonstrate thread-based parallel processing"""
    print("\n=== Threaded Performance Demo ===\n")
    
    client = ThreadedTimberDBClient()
    
    components = [
        "authentication", "payment-processor", "database", 
        "api-gateway", "user-service", "order-service"
    ]
    
    time_range = ("2024-03-15T14:00:00Z", "2024-03-15T16:00:00Z")
    
    start_time = time.time()
    results = client.parallel_component_analysis(components, time_range)
    threaded_duration = time.time() - start_time
    
    print(f"Threaded component analysis: {threaded_duration:.3f}s")
    print(f"Components analyzed: {len(results)}")
    
    for component, result in results.items():
        entry_count = len(result.get("entries", []))
        print(f"  {component}: {entry_count} entries")

if __name__ == "__main__":
    # Run async performance demo
    asyncio.run(performance_demo())
    
    # Run threaded performance demo
    threaded_performance_demo()
```

### Memory and Resource Management

Effective memory management becomes crucial when processing large volumes of log data or running long-lived monitoring applications. Understanding how to manage memory consumption, handle large result sets, and optimize resource utilization helps build applications that can scale to enterprise workloads.

**Result Set Streaming** involves processing query results in chunks rather than loading entire result sets into memory simultaneously. This approach enables analysis of datasets that exceed available memory while maintaining predictable resource consumption patterns.

**Memory-aware Query Sizing** adjusts query limits and timeouts based on available system resources and expected result set sizes. This dynamic approach helps prevent out-of-memory conditions while maximizing query efficiency for available resources.

**Resource Pooling and Cleanup** ensures that long-running applications properly manage connection pools, release unused resources, and implement appropriate garbage collection strategies to maintain stable memory usage over time.

```python
#!/usr/bin/env python3
"""
Memory-efficient TimberDB query processing
Demonstrates streaming, chunking, and resource management patterns
"""

import gc
import psutil
import os
from typing import Iterator, Dict, List, Generator
import json
from datetime import datetime, timedelta

class MemoryEfficientProcessor:
    def __init__(self, memory_limit_mb: int = 512):
        self.memory_limit_bytes = memory_limit_mb * 1024 * 1024
        self.processed_count = 0
        
    def get_memory_usage(self) -> int:
        """Get current memory usage in bytes"""
        process = psutil.Process(os.getpid())
        return process.memory_info().rss
    
    def check_memory_usage(self) -> bool:
        """Check if memory usage is within limits"""
        current_usage = self.get_memory_usage()
        return current_usage < self.memory_limit_bytes
    
    def process_large_dataset_streaming(self, 
                                      start_time: datetime, 
                                      end_time: datetime,
                                      chunk_minutes: int = 30) -> Iterator[Dict]:
        """Process large time range using streaming approach"""
        
        current_time = start_time
        chunk_count = 0
        
        while current_time < end_time:
            # Calculate chunk boundaries
            chunk_end = min(current_time + timedelta(minutes=chunk_minutes), end_time)
            
            # Build query for this chunk
            query = {
                "filter": {
                    "time_range": [
                        current_time.strftime("%Y-%m-%dT%H:%M:%SZ"),
                        chunk_end.strftime("%Y-%m-%dT%H:%M:%SZ")
                    ],
                    "tags": {}
                },
                "limit": 1000,
                "timeout_seconds": 30
            }
            
            # Execute query
            print(f"Processing chunk {chunk_count + 1}: {current_time} to {chunk_end}")
            result = self.execute_query_with_retry(query)
            
            # Process entries one by one to minimize memory usage
            for entry in result.get("entries", []):
                yield entry
                self.processed_count += 1
                
                # Check memory usage periodically
                if self.processed_count % 100 == 0:
                    if not self.check_memory_usage():
                        print("Memory limit exceeded, forcing garbage collection")
                        gc.collect()
                        
                        if not self.check_memory_usage():
                            raise MemoryError("Unable to stay within memory limits")
            
            # Clean up chunk data
            del result
            gc.collect()
            
            current_time = chunk_end
            chunk_count += 1
            
        print(f"Streaming processing complete: {self.processed_count} entries")
    
    def execute_query_with_retry(self, query: Dict, max_retries: int = 3) -> Dict:
        """Execute query with retry logic and error handling"""
        import requests
        import time
        
        for attempt in range(max_retries):
            try:
                response = requests.post(
                    "http://localhost:7777/api/v1/query/execute",
                    json=query,
                    timeout=60
                )
                response.raise_for_status()
                return response.json()
                
            except requests.exceptions.RequestException as e:
                if attempt < max_retries - 1:
                    wait_time = 2 ** attempt  # Exponential backoff
                    print(f"Query failed (attempt {attempt + 1}), retrying in {wait_time}s: {e}")
                    time.sleep(wait_time)
                else:
                    print(f"Query failed after {max_retries} attempts: {e}")
                    return {"entries": [], "matched_entries": 0, "error": str(e)}
    
    def process_with_memory_monitoring(self, processing_func, *args, **kwargs):
        """Wrapper that monitors memory usage during processing"""
        initial_memory = self.get_memory_usage()
        print(f"Initial memory usage: {initial_memory / 1024 / 1024:.1f} MB")
        
        try:
            result = processing_func(*args, **kwargs)
            
            final_memory = self.get_memory_usage()
            memory_delta = final_memory - initial_memory
            
            print(f"Final memory usage: {final_memory / 1024 / 1024:.1f} MB")
            print(f"Memory delta: {memory_delta / 1024 / 1024:.1f} MB")
            
            return result
            
        except Exception as e:
            error_memory = self.get_memory_usage()
            print(f"Error occurred at memory usage: {error_memory / 1024 / 1024:.1f} MB")
            raise

# Example of memory-efficient batch processing
def memory_efficient_analysis_example():
    """Demonstrate memory-efficient analysis patterns"""
    
    processor = MemoryEfficientProcessor(memory_limit_mb=256)  # 256MB limit
    
    # Define analysis time range
    start_time = datetime(2024, 3, 15, 0, 0, 0)
    end_time = datetime(2024, 3, 15, 23, 59, 59)
    
    # Statistics tracking
    error_counts = {}
    component_stats = {}
    hourly_distribution = [0] * 24
    
    print("Starting memory-efficient log analysis...")
    
    # Process data in streaming fashion
    entry_count = 0
    for entry in processor.process_large_dataset_streaming(start_time, end_time):
        # Extract timestamp and categorize by hour
        timestamp = datetime.fromisoformat(entry["timestamp"].replace("Z", "+00:00"))
        hour = timestamp.hour
        hourly_distribution[hour] += 1
        
        # Count errors by component
        tags = entry.get("tags", {})
        level = tags.get("level", "unknown")
        component = tags.get("component", "unknown")
        
        if level == "error":
            error_counts[component] = error_counts.get(component, 0) + 1
        
        # Track component activity
        if component not in component_stats:
            component_stats[component] = {"total": 0, "errors": 0}
        
        component_stats[component]["total"] += 1
        if level == "error":
            component_stats[component]["errors"] += 1
        
        entry_count += 1
        
        # Periodic progress reporting
        if entry_count % 1000 == 0:
            memory_mb = processor.get_memory_usage() / 1024 / 1024
            print(f"Processed {entry_count} entries, memory usage: {memory_mb:.1f} MB")
    
    # Generate analysis report
    print(f"\n=== Analysis Complete ===")
    print(f"Total entries processed: {entry_count}")
    print(f"Peak memory usage: {processor.get_memory_usage() / 1024 / 1024:.1f} MB")
    
    print(f"\nTop error sources:")
    for component, count in sorted(error_counts.items(), key=lambda x: x[1], reverse=True)[:5]:
        print(f"  {component}: {count} errors")
    
    print(f"\nComponent error rates:")
    for component, stats in component_stats.items():
        if stats["total"] > 0:
            error_rate = (stats["errors"] / stats["total"]) * 100
            print(f"  {component}: {error_rate:.2f}% ({stats['errors']}/{stats['total']})")
    
    print(f"\nHourly activity distribution:")
    for hour, count in enumerate(hourly_distribution):
        if count > 0:
            print(f"  {hour:02d}:00-{hour:02d}:59: {count} entries")

if __name__ == "__main__":
    memory_efficient_analysis_example()
```

### Index Strategy and Query Patterns

Understanding how TimberDB's indexing system works enables you to structure both your data and your queries for optimal performance. The indexing strategy affects not just query speed, but also storage efficiency and overall system throughput.

**Time-based Index Optimization** takes advantage of TimberDB's natural time-series indexing to structure queries that can efficiently skip irrelevant data blocks. Queries that specify precise time ranges will almost always outperform those that scan entire datasets, so time-based filtering should be used whenever possible.

**Tag Index Utilization** leverages the structured metadata indexes to enable efficient filtering on categorical data. The most effective tag-based queries use specific tag values rather than broad categories, and combine multiple tags to create highly selective filter conditions.

**Content Search Optimization** balances the power of full-text search with performance considerations by combining text searches with other filter types to reduce the amount of content that must be analyzed. Using specific search terms rather than common words dramatically improves search performance.

```bash
# Demonstrate optimal query patterns for different use cases

# 1. Time-series analysis with precise time windows
echo "=== Optimal Time-series Queries ==="

# Good: Specific time range with selective tags
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:30:00Z", "2024-03-15T14:45:00Z"],
      "tags": {
        "level": "error",
        "component": "payment-processor"
      }
    },
    "limit": 500,
    "timeout_seconds": 15
  }'

# 2. Tag-based optimization patterns
echo "=== Tag Index Optimization ==="

# Good: Multiple specific tags for high selectivity
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "transaction_id": "txn-12345",
        "user_id": "user-67890",
        "payment_method": "credit_card"
      }
    },
    "limit": 100,
    "timeout_seconds": 10
  }'

# Good: Error analysis with component specificity
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T12:00:00Z", "2024-03-15T18:00:00Z"],
      "tags": {
        "level": "error",
        "component": "database",
        "error_code": "CONNECTION_TIMEOUT"
      }
    },
    "limit": 200
  }'

# 3. Content search optimization
echo "=== Content Search Optimization ==="

# Good: Specific error codes or identifiers
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T16:00:00Z"],
      "tags": {
        "component": "authentication"
      },
      "message_contains": "AUTH_ERROR_001"
    },
    "limit": 100
  }'

# Good: Business event tracking with specific identifiers
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "tags": {
        "business_event": "order_created"
      },
      "message_contains": "order_id"
    },
    "limit": 500
  }'

# 4. Progressive query refinement
echo "=== Progressive Query Refinement ==="

# Step 1: Broad search to understand scope
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:00:00Z", "2024-03-15T16:00:00Z"],
      "tags": {
        "level": "error"
      }
    },
    "limit": 100
  }' > broad_errors.json

# Step 2: Refined search based on initial results
curl -X POST http://localhost:7777/api/v1/query/execute \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "time_range": ["2024-03-15T14:30:00Z", "2024-03-15T14:45:00Z"],
      "tags": {
        "level": "error",
        "component": "payment-processor"
      }
    },
    "limit": 200
  }' > refined_errors.json

# 5. Batch processing for large datasets
echo "=== Efficient Batch Processing ==="

# Process 6-hour period in 1-hour chunks
for hour_offset in 0 1 2 3 4 5; do
    START_TIME=$(date -u -d "2024-03-15 12:00:00 + ${hour_offset} hours" +%Y-%m-%dT%H:%M:%SZ)
    END_TIME=$(date -u -d "2024-03-15 12:00:00 + $((hour_offset + 1)) hours" +%Y-%m-%dT%H:%M:%SZ)
    
    echo "Processing hour ${hour_offset}: ${START_TIME} to ${END_TIME}"
    
    curl -X POST http://localhost:7777/api/v1/query/execute \
      -H "Content-Type: application/json" \
      -d "{
        \"filter\": {
          \"time_range\": [\"${START_TIME}\", \"${END_TIME}\"],
          \"tags\": {
            \"level\": \"error\"
          }
        },
        \"limit\": 1000,
        \"timeout_seconds\": 30
      }" > "errors_hour_${hour_offset}.json"
    
    # Small delay to prevent overwhelming the server
    sleep 1
done

echo "Batch processing complete"
```

---

## Monitoring and Observability

Effective monitoring of TimberDB deployments requires understanding both the system's internal health metrics and the patterns of log data being processed. A comprehensive monitoring strategy combines real-time health checks, performance metrics, capacity tracking, and alerting to ensure reliable operation and early detection of potential issues.

### Health Check Implementation

TimberDB provides built-in health check endpoints that report on various aspects of system health. Understanding how to interpret these health indicators and integrate them into your monitoring infrastructure enables proactive management of your log analytics environment.

**System Health Monitoring** involves regularly checking the overall status of TimberDB nodes, including storage engine health, network connectivity, and query processing capabilities. The health check endpoint provides structured information about component status that can be integrated into monitoring dashboards and alerting systems.

**Component-level Health Assessment** examines the health of individual TimberDB subsystems including storage, networking, query processing, and cluster consensus mechanisms. Understanding the health status of each component helps identify specific areas that may need attention or optimization.

**Cluster Health Evaluation** in distributed deployments requires monitoring the health of individual nodes as well as the overall cluster state, including leader election status, consensus health, and data replication consistency.

```bash
# Comprehensive health monitoring script
#!/bin/bash

# Health monitoring for TimberDB deployment
TIMBERDB_URL="http://localhost:7777"
MONITORING_LOG="/var/log/timberdb-monitoring.log"
ALERT_THRESHOLD_ERROR_RATE=0.05  # 5% error rate threshold

echo "=== TimberDB Health Check - $(date) ===" | tee -a "$MONITORING_LOG"

# 1. Basic connectivity and health check
echo "Checking basic connectivity..." | tee -a "$MONITORING_LOG"
if curl -s --connect-timeout 5 "$TIMBERDB_URL/health" > /tmp/health_check.json; then
    echo "✓ TimberDB is responding" | tee -a "$MONITORING_LOG"
    
    # Parse health check response
    STATUS=$(jq -r '.status' /tmp/health_check.json 2>/dev/null || echo "unknown")
    UPTIME=$(jq -r '.uptime_seconds' /tmp/health_check.json 2>/dev/null || echo "unknown")
    VERSION=$(jq -r '.version' /tmp/health_check.json 2>/dev/null || echo "unknown")
    
    echo "  Status: $STATUS" | tee -a "$MONITORING_LOG"
    echo "  Uptime: $UPTIME seconds" | tee -a "$MONITORING_LOG"
    echo "  Version: $VERSION" | tee -a "$MONITORING_LOG"
    
    # Check component health
    STORAGE_STATUS=$(jq -r '.storage.status' /tmp/health_check.json 2>/dev/null || echo "unknown")
    QUERY_STATUS=$(jq -r '.query_engine.status' /tmp/health_check.json 2>/dev/null || echo "unknown")
    NETWORK_STATUS=$(jq -r '.network.status // "not_applicable"' /tmp/health_check.json 2>/dev/null || echo "unknown")
    
    echo "  Storage: $STORAGE_STATUS" | tee -a "$MONITORING_LOG"
    echo "  Query Engine: $QUERY_STATUS" | tee -a "$MONITORING_LOG"
    echo "  Network: $NETWORK_STATUS" | tee -a "$MONITORING_LOG"
    
    # Alert on unhealthy components
    if [[ "$STORAGE_STATUS" != "healthy" ]]; then
        echo "⚠️  ALERT: Storage component unhealthy" | tee -a "$MONITORING_LOG"
    fi
    
    if [[ "$QUERY_STATUS" != "healthy" ]]; then
        echo "⚠️  ALERT: Query engine unhealthy" | tee -a "$MONITORING_LOG"
    fi
    
else
    echo "❌ CRITICAL: TimberDB not responding" | tee -a "$MONITORING_LOG"
    exit 1
fi

# 2. Performance metrics check
echo -e "\nChecking performance metrics..." | tee -a "$MONITORING_LOG"
if curl -s --connect-timeout 5 "$TIMBERDB_URL/metrics" > /tmp/metrics.json; then
    echo "✓ Metrics endpoint responding" | tee -a "$MONITORING_LOG"
    
    # Extract key metrics
    TOTAL_REQUESTS=$(jq -r '.total_requests' /tmp/metrics.json 2>/dev/null || echo "0")
    ACTIVE_QUERIES=$(jq -r '.active_queries' /tmp/metrics.json 2>/dev/null || echo "0")
    STORAGE_SIZE=$(jq -r '.storage_size_bytes' /tmp/metrics.json 2>/dev/null || echo "0")
    PARTITION_COUNT=$(jq -r '.partition_count' /tmp/metrics.json 2>/dev/null || echo "0")
    ERROR_RATE=$(jq -r '.error_rate' /tmp/metrics.json 2>/dev/null || echo "0")
    
    echo "  Total Requests: $TOTAL_REQUESTS" | tee -a "$MONITORING_LOG"
    echo "  Active Queries: $ACTIVE_QUERIES" | tee -a "$MONITORING_LOG"
    echo "  Storage Size: $(numfmt --to=iec $STORAGE_SIZE)" | tee -a "$MONITORING_LOG"
    echo "  Partitions: $PARTITION_COUNT" | tee -a "$MONITORING_LOG"
    echo "  Error Rate: $ERROR_RATE" | tee -a "$MONITORING_LOG"
    
    # Check for concerning metrics
    if (( $(echo "$ERROR_RATE > $ALERT_THRESHOLD_ERROR_RATE" | bc -l) )); then
        echo "⚠️  ALERT: High error rate detected: $ERROR_RATE" | tee -a "$MONITORING_LOG"
    fi
    
    if [[ "$ACTIVE_QUERIES" -gt 20 ]]; then
        echo "⚠️  WARNING: High number of active queries: $ACTIVE_QUERIES" | tee -a "$MONITORING_LOG"
    fi
else
    echo "❌ Metrics endpoint not responding" | tee -a "$MONITORING_LOG"
fi

# 3. Functional testing with simple queries
echo -e "\nPerforming functional tests..." | tee -a "$MONITORING_LOG"

# Test basic query functionality
TEST_QUERY='{
  "filter": {
    "tags": {}
  },
  "limit": 1,
  "timeout_seconds": 5
}'

if curl -s --connect-timeout 10 -X POST "$TIMBERDB_URL/api/v1/query/execute" \
   -H "Content-Type: application/json" \
   -d "$TEST_QUERY" > /tmp/test_query.json; then
    
    QUERY_ERROR=$(jq -r '.error // "none"' /tmp/test_query.json 2>/dev/null || echo "parse_error")
    
    if [[ "$QUERY_ERROR" == "none" ]]; then
        echo "✓ Basic query functionality working" | tee -a "$MONITORING_LOG"
    else
        echo "❌ Query functionality error: $QUERY_ERROR" | tee -a "$MONITORING_LOG"
    fi
else
    echo "❌ Query endpoint not responding" | tee -a "$MONITORING_LOG"
fi

# 4. Storage capacity monitoring
echo -e "\nChecking storage capacity..." | tee -a "$MONITORING_LOG"

# Check disk usage of data directory (assuming /var/lib/timberdb)
DATA_DIR="/var/lib/timberdb"
if [[ -d "$DATA_DIR" ]]; then
    DISK_USAGE=$(df -h "$DATA_DIR" | awk 'NR==2 {print $5}' | sed 's/%//')
    DISK_AVAILABLE=$(df -h "$DATA_DIR" | awk 'NR==2 {print $4}')
    
    echo "  Disk usage: ${DISK_USAGE}%" | tee -a "$MONITORING_LOG"
    echo "  Available space: $DISK_AVAILABLE" | tee -a "$MONITORING_LOG"
    
    if [[ "$DISK_USAGE" -gt 85 ]]; then
        echo "⚠️  ALERT: High disk usage: ${DISK_USAGE}%" | tee -a "$MONITORING_LOG"
    fi
    
    if [[ "$DISK_USAGE" -gt 95 ]]; then
        echo "🔥 CRITICAL: Very high disk usage: ${DISK_USAGE}%" | tee -a "$MONITORING_LOG"
    fi
else
    echo "⚠️  Data directory not found: $DATA_DIR" | tee -a "$MONITORING_LOG"
fi

# 5. Recent error analysis
echo -e "\nAnalyzing recent error patterns..." | tee -a "$MONITORING_LOG"

# Query for recent errors (last 10 minutes)
RECENT_ERRORS_QUERY='{
  "filter": {
    "time_range": ["'$(date -u -d '10 minutes ago' +%Y-%m-%dT%H:%M:%SZ)'", "'$(date -u +%Y-%m-%dT%H:%M:%SZ)'"],
    "tags": {
      "level": "error"
    }
  },
  "limit": 100,
  "timeout_seconds": 15
}'

if curl -s -X POST "$TIMBERDB_URL/api/v1/query/execute" \
   -H "Content-Type: application/json" \
   -d "$RECENT_ERRORS_QUERY" > /tmp/recent_errors.json; then
    
    ERROR_COUNT=$(jq -r '.matched_entries // 0' /tmp/recent_errors.json 2>/dev/null || echo "0")
    echo "  Recent errors (10 min): $ERROR_COUNT" | tee -a "$MONITORING_LOG"
    
    if [[ "$ERROR_COUNT" -gt 50 ]]; then
        echo "⚠️  ALERT: High recent error count: $ERROR_COUNT" | tee -a "$MONITORING_LOG"
        
        # Analyze error patterns
        if command -v jq >/dev/null 2>&1; then
            echo "  Top error components:" | tee -a "$MONITORING_LOG"
            jq -r '.entries[]?.tags.component // "unknown"' /tmp/recent_errors.json 2>/dev/null | \
                sort | uniq -c | sort -nr | head -3 | \
                while read count component; do
                    echo "    $component: $count errors" | tee -a "$MONITORING_LOG"
                done
        fi
    fi
else
    echo "❌ Unable to query recent errors" | tee -a "$MONITORING_LOG"
fi

# 6. Generate summary and cleanup
echo -e "\n=== Health Check Summary ===" | tee -a "$MONITORING_LOG"

# Count alerts and warnings from this run
ALERT_COUNT=$(grep -c "ALERT\|CRITICAL" /tmp/health_check_current.log 2>/dev/null || echo "0")
WARNING_COUNT=$(grep -c "WARNING" /tmp/health_check_current.log 2>/dev/null || echo "0")

if [[ "$ALERT_COUNT" -gt 0 ]]; then
    echo "🚨 $ALERT_COUNT alerts detected" | tee -a "$MONITORING_LOG"
elif [[ "$WARNING_COUNT" -gt 0 ]]; then
    echo "⚠️  $WARNING_COUNT warnings detected" | tee -a "$MONITORING_LOG"
else
    echo "✅ All systems healthy" | tee -a "$MONITORING_LOG"
fi

echo "Health check completed at $(date)" | tee -a "$MONITORING_LOG"
echo "" | tee -a "$MONITORING_LOG"

# Cleanup temporary files
rm -f /tmp/health_check.json /tmp/metrics.json /tmp/test_query.json /tmp/recent_errors.json

# Exit with appropriate code
if [[ "$ALERT_COUNT" -gt 0 ]]; then
    exit 1
else
    exit 0
fi
```

### Performance Metrics and Monitoring

Performance monitoring provides insights into TimberDB's operational characteristics, helping identify optimization opportunities and capacity planning requirements. Effective performance monitoring tracks query response times, throughput metrics, resource utilization, and system bottlenecks.

**Query Performance Analysis** involves tracking query execution times, success rates, and resource consumption patterns. Understanding which queries perform well and which may need optimization helps maintain good system performance as data volumes and query loads increase.

**Throughput and Capacity Metrics** monitor the system's ability to handle incoming log data and query workloads. These metrics help with capacity planning and identifying when system resources may need to be scaled or optimized.

**Resource Utilization Tracking** examines CPU, memory, disk, and network usage patterns to identify bottlenecks and optimization opportunities. Understanding resource consumption patterns helps with both performance tuning and infrastructure planning.

```python
#!/usr/bin/env python3
"""
Comprehensive performance monitoring for TimberDB
Tracks metrics, analyzes trends, and generates performance reports
"""

import time
import json
import requests
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Tuple
from collections import defaultdict, deque
import statistics
import sys

class TimberDBPerformanceMonitor:
    def __init__(self, base_url: str = "http://localhost:7777"):
        self.base_url = base_url
        self.query_times = deque(maxlen=1000)  # Last 1000 query times
        self.query_success_rate = deque(maxlen=100)  # Last 100 query results
        self.system_metrics_history = deque(maxlen=288)  # 24 hours at 5-min intervals
        
    def execute_performance_query(self, query: Dict, 
                                measure_performance: bool = True) -> Tuple[Dict, float]:
        """Execute query and measure performance"""
        start_time = time.time()
        
        try:
            response = requests.post(
                f"{self.base_url}/api/v1/query/execute",
                json=query,
                timeout=60
            )
            response.raise_for_status()
            result = response.json()
            
            execution_time = time.time() - start_time
            
            if measure_performance:
                self.query_times.append(execution_time)
                self.query_success_rate.append(True)
            
            return result, execution_time
            
        except Exception as e:
            execution_time = time.time() - start_time
            
            if measure_performance:
                self.query_times.append(execution_time)
                self.query_success_rate.append(False)
            
            return {"error": str(e), "entries": []}, execution_time
    
    def get_system_metrics(self) -> Dict:
        """Retrieve current system metrics"""
        try:
            response = requests.get(f"{self.base_url}/metrics", timeout=10)
            response.raise_for_status()
            metrics = response.json()
            
            # Add timestamp
            metrics["timestamp"] = datetime.utcnow().isoformat()
            
            # Store in history
            self.system_metrics_history.append(metrics)
            
            return metrics
            
        except Exception as e:
            return {"error": str(e), "timestamp": datetime.utcnow().isoformat()}
    
    def analyze_query_performance(self) -> Dict:
        """Analyze recent query performance"""
        if not self.query_times:
            return {"error": "No query performance data available"}
        
        times = list(self.query_times)
        success_count = sum(self.query_success_rate)
        total_queries = len(self.query_success_rate)
        
        return {
            "query_count": len(times),
            "avg_response_time": statistics.mean(times),
            "median_response_time": statistics.median(times),
            "p95_response_time": times[int(len(times) * 0.95)] if times else 0,
            "p99_response_time": times[int(len(times) * 0.99)] if times else 0,
            "min_response_time": min(times),
            "max_response_time": max(times),
            "success_rate": success_count / max(total_queries, 1),
            "failed_queries": total_queries - success_count
        }
    
    def benchmark_query_types(self) -> Dict:
        """Benchmark different types of queries"""
        benchmark_queries = {
            "simple_tag_filter": {
                "filter": {"tags": {"level": "error"}},
                "limit": 100,
                "timeout_seconds": 10
            },
            "time_range_filter": {
                "filter": {
                    "time_range": [
                        (datetime.utcnow() - timedelta(hours=1)).strftime("%Y-%m-%dT%H:%M:%SZ"),
                        datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")
                    ],
                    "tags": {}
                },
                "limit": 500,
                "timeout_seconds": 15
            },
            "complex_filter": {
                "filter": {
                    "time_range": [
                        (datetime.utcnow() - timedelta(hours=2)).strftime("%Y-%m-%dT%H:%M:%SZ"),
                        datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ")
                    ],
                    "tags": {"level": "error", "component": "authentication"},
                    "message_contains": "failed"
                },
                "limit": 200,
                "timeout_seconds": 20
            },
            "text_search": {
                "filter": {
                    "tags": {},
                    "message_contains": "timeout"
                },
                "limit": 100,
                "timeout_seconds": 25
            }
        }
        
        results = {}
        
        print("Running query performance benchmarks...")
        
        for query_type, query in benchmark_queries.items():
            print(f"  Testing {query_type}...")
            
            # Run query multiple times for accuracy
            times = []
            success_count = 0
            
            for i in range(3):  # 3 iterations per query type
                result, exec_time = self.execute_performance_query(
                    query, measure_performance=False
                )
                times.append(exec_time)
                
                if "error" not in result:
                    success_count += 1
                
                time.sleep(0.5)  # Brief pause between iterations
            
            results[query_type] = {
                "avg_time": statistics.mean(times),
                "min_time": min(times),
                "max_time": max(times),
                "success_rate": success_count / len(times),
                "iterations": len(times)
            }
        
        return results
    
    def analyze_system_trends(self) -> Dict:
        """Analyze system performance trends"""
        if len(self.system_metrics_history) < 2:
            return {"error": "Insufficient historical data"}
        
        # Extract metrics over time
        timestamps = []
        request_counts = []
        error_rates = []
        active_queries = []
        storage_sizes = []
        
        for metrics in self.system_metrics_history:
            if "error" not in metrics:
                timestamps.append(metrics.get("timestamp"))
                request_counts.append(metrics.get("total_requests", 0))
                error_rates.append(metrics.get("error_rate", 0))
                active_queries.append(metrics.get("active_queries", 0))
                storage_sizes.append(metrics.get("storage_size_bytes", 0))
        
        if not request_counts:
            return {"error": "No valid metrics data"}
        
        # Calculate trends
        analysis = {
            "data_points": len(request_counts),
            "time_span_hours": len(request_counts) * 5 / 60,  # 5-minute intervals
            "request_rate_trend": self._calculate_trend(request_counts),
            "error_rate_trend": self._calculate_trend(error_rates),
            "active_queries_trend": self._calculate_trend(active_queries),
            "storage_growth_trend": self._calculate_trend(storage_sizes),
            "current_metrics": {
                "total_requests": request_counts[-1],
                "error_rate": error_rates[-1],
                "active_queries": active_queries[-1],
                "storage_size_gb": storage_sizes[-1] / (1024**3)
            }
        }
        
        return analysis
    
    def _calculate_trend(self, values: List[float]) -> str:
        """Calculate trend direction from a series of values"""
        if len(values) < 2:
            return "insufficient_data"
        
        # Simple linear trend calculation
        x = list(range(len(values)))
        n = len(values)
        
        sum_x = sum(x)
        sum_y = sum(values)
        sum_xy = sum(x[i] * values[i] for i in range(n))
        sum_x2 = sum(x[i] ** 2 for i in range(n))
        
        # Calculate slope
        slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x ** 2)
        
        if slope > 0.1:
            return "increasing"
        elif slope < -0.1:
            return "decreasing"
        else:
            return "stable"
    
    def generate_performance_report(self) -> str:
        """Generate comprehensive performance report"""
        report_time = datetime.utcnow().strftime("%Y-%m-%d %H:%M:%S UTC")
        
        # Collect all performance data
        system_metrics = self.get_system_metrics()
        query_analysis = self.analyze_query_performance()
        system_trends = self.analyze_system_trends()
        benchmark_results = self.benchmark_query_types()
        
        # Generate report
        report = f"""
=== TimberDB Performance Report ===
Generated: {report_time}

1. SYSTEM METRICS
-----------------
"""
        
        if "error" not in system_metrics:
            report += f"""
Uptime: {system_metrics.get('uptime_seconds', 0)} seconds
Total Requests: {system_metrics.get('total_requests', 0):,}
Active Queries: {system_metrics.get('active_queries', 0)}
Storage Size: {system_metrics.get('storage_size_bytes', 0) / (1024**3):.2f} GB
Partition Count: {system_metrics.get('partition_count', 0)}
Error Rate: {system_metrics.get('error_rate', 0):.4f}
"""
        else:
            report += f"Error retrieving system metrics: {system_metrics['error']}\n"
        
        report += """
2. QUERY PERFORMANCE ANALYSIS
-----------------------------
"""
        
        if "error" not in query_analysis:
            report += f"""
Queries Analyzed: {query_analysis['query_count']}
Average Response Time: {query_analysis['avg_response_time']:.3f}s
Median Response Time: {query_analysis['median_response_time']:.3f}s
95th Percentile: {query_analysis['p95_response_time']:.3f}s
99th Percentile: {query_analysis['p99_response_time']:.3f}s
Success Rate: {query_analysis['success_rate']:.2%}
Failed Queries: {query_analysis['failed_queries']}
"""
        else:
            report += f"Error in query analysis: {query_analysis['error']}\n"
        
        report += """
3. SYSTEM TRENDS
----------------
"""
        
        if "error" not in system_trends:
            report += f"""
Data Points: {system_trends['data_points']}
Time Span: {system_trends['time_span_hours']:.1f} hours
Request Rate Trend: {system_trends['request_rate_trend']}
Error Rate Trend: {system_trends['error_rate_trend']}
Active Queries Trend: {system_trends['active_queries_trend']}
Storage Growth Trend: {system_trends['storage_growth_trend']}
"""
        else:
            report += f"Error in trend analysis: {system_trends['error']}\n"
        
        report += """
4. QUERY BENCHMARKS
-------------------
"""
        
        for query_type, results in benchmark_results.items():
            report += f"""
{query_type.replace('_', ' ').title()}:
  Average Time: {results['avg_time']:.3f}s
  Min/Max Time: {results['min_time']:.3f}s / {results['max_time']:.3f}s
  Success Rate: {results['success_rate']:.2%}
"""
        
        report += """
5. RECOMMENDATIONS
------------------
"""
        
        # Generate recommendations based on analysis
        recommendations = []
        
        if "error" not in query_analysis:
            if query_analysis['avg_response_time'] > 1.0:
                recommendations.append("- Consider optimizing slow queries or adding more specific filters")
            
            if query_analysis['success_rate'] < 0.95:
                recommendations.append("- Investigate query failures and timeout issues")
        
        if "error" not in system_metrics:
            if system_metrics.get('error_rate', 0) > 0.01:
                recommendations.append("- High error rate detected, investigate system logs")
            
            if system_metrics.get('active_queries', 0) > 20:
                recommendations.append("- High concurrent query load, consider query optimization")
        
        if not recommendations:
            recommendations.append("- System performance appears optimal")
        
        for rec in recommendations:
            report += f"{rec}\n"
        
        report += f"\n=== End of Report ===\n"
        
        return report
    
    def start_continuous_monitoring(self, interval_minutes: int = 5):
        """Start continuous performance monitoring"""
        print(f"Starting continuous performance monitoring (interval: {interval_minutes} minutes)")
        print("Press Ctrl+C to stop")
        
        try:
            while True:
                print(f"\n--- Monitoring Cycle: {datetime.utcnow()} ---")
                
                # Collect system metrics
                metrics = self.get_system_metrics()
                if "error" not in metrics:
                    print(f"Active Queries: {metrics.get('active_queries', 0)}")
                    print(f"Error Rate: {metrics.get('error_rate', 0):.4f}")
                    print(f"Storage: {metrics.get('storage_size_bytes', 0) / (1024**3):.2f} GB")
                
                # Perform test query to measure response time
                test_query = {
                    "filter": {"tags": {}},
                    "limit": 1,
                    "timeout_seconds": 5
                }
                
                _, response_time = self.execute_performance_query(test_query)
                print(f"Test Query Response Time: {response_time:.3f}s")
                
                # Sleep until next monitoring cycle
                time.sleep(interval_minutes * 60)
                
        except KeyboardInterrupt:
            print("\nMonitoring stopped by user")
        except Exception as e:
            print(f"Monitoring error: {e}")

# Example usage
if __name__ == "__main__":
    monitor = TimberDBPerformanceMonitor()
    
    if len(sys.argv) > 1 and sys.argv[1] == "continuous":
        # Start continuous monitoring
        monitor.start_continuous_monitoring()
    else:
        # Generate single performance report
        report = monitor.generate_performance_report()
        print(report)
        
        # Optionally save to file
        timestamp = datetime.utcnow().strftime("%Y%m%d_%H%M%S")
        filename = f"timberdb_performance_report_{timestamp}.txt"
        
        with open(filename, 'w') as f:
            f.write(report)
        
        print(f"Report saved to: {filename}")
```