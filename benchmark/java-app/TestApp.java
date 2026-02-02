// benchmark/java-app/TestApp.java
//
// Simple Java application for benchmarking rJMX-Exporter vs jmx_exporter
// This app generates predictable memory and thread activity for consistent metrics.

import java.lang.management.ManagementFactory;
import java.lang.management.MemoryMXBean;
import java.lang.management.ThreadMXBean;
import java.lang.management.GarbageCollectorMXBean;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;
import java.util.concurrent.atomic.AtomicLong;
import com.sun.net.httpserver.HttpServer;
import java.net.InetSocketAddress;

/**
 * Benchmark test application that:
 * - Allocates configurable memory (default 50MB)
 * - Runs multiple threads for thread metrics
 * - Periodically triggers GC for GC metrics
 * - Provides HTTP endpoints for health checks
 */
public class TestApp {
    // Configuration via environment variables
    private static final int MEMORY_MB = Integer.parseInt(
        System.getenv().getOrDefault("MEMORY_MB", "50")
    );
    private static final int WORKER_THREADS = Integer.parseInt(
        System.getenv().getOrDefault("WORKER_THREADS", "5")
    );

    private static final Random random = new Random();
    private static final AtomicLong requestCount = new AtomicLong(0);

    // Memory holder to generate consistent heap usage
    private static final List<byte[]> memoryHolder = new ArrayList<>();

    public static void main(String[] args) throws Exception {
        System.out.println("=== rJMX-Exporter Benchmark Test Application ===");
        System.out.println("Configuration:");
        System.out.println("  Memory allocation: " + MEMORY_MB + " MB");
        System.out.println("  Worker threads: " + WORKER_THREADS);
        System.out.println("  Jolokia endpoint: http://localhost:8778/jolokia");
        System.out.println();

        // Allocate memory in chunks to simulate real application usage
        allocateMemory();

        // Start worker threads for thread metrics
        startWorkerThreads();

        // Start HTTP server
        startHttpServer();

        // Start GC trigger thread for GC metrics
        startGcTrigger();

        // Print JVM info
        printJvmInfo();

        System.out.println("\nApplication started successfully!");
        System.out.println("HTTP server: http://localhost:8080");
        System.out.println("Jolokia: http://localhost:8778/jolokia");

        // Keep application running
        Thread.currentThread().join();
    }

    private static void allocateMemory() {
        System.out.println("Allocating " + MEMORY_MB + " MB of memory...");
        int chunkSize = 1024 * 1024; // 1 MB chunks
        for (int i = 0; i < MEMORY_MB; i++) {
            byte[] chunk = new byte[chunkSize];
            // Touch the memory to ensure it's actually allocated
            for (int j = 0; j < chunkSize; j += 4096) {
                chunk[j] = (byte) i;
            }
            memoryHolder.add(chunk);
        }
        System.out.println("Memory allocated: " + memoryHolder.size() + " MB");
    }

    private static void startWorkerThreads() {
        System.out.println("Starting " + WORKER_THREADS + " worker threads...");
        for (int i = 0; i < WORKER_THREADS; i++) {
            final int threadId = i;
            Thread worker = new Thread(() -> {
                while (true) {
                    try {
                        // Simulate some work
                        double result = 0;
                        for (int j = 0; j < 1000; j++) {
                            result += Math.sqrt(random.nextDouble());
                        }
                        Thread.sleep(100 + random.nextInt(100));
                    } catch (InterruptedException e) {
                        break;
                    }
                }
            }, "worker-" + threadId);
            worker.setDaemon(true);
            worker.start();
        }
    }

    private static void startHttpServer() throws Exception {
        HttpServer server = HttpServer.create(new InetSocketAddress(8080), 0);

        // Root endpoint
        server.createContext("/", exchange -> {
            long count = requestCount.incrementAndGet();
            String response = String.format(
                "{\"app\":\"benchmark-test\",\"requests\":%d,\"memory_mb\":%d,\"threads\":%d}",
                count, MEMORY_MB, WORKER_THREADS
            );
            exchange.getResponseHeaders().set("Content-Type", "application/json");
            exchange.sendResponseHeaders(200, response.length());
            exchange.getResponseBody().write(response.getBytes());
            exchange.close();
        });

        // Health endpoint
        server.createContext("/health", exchange -> {
            String response = "{\"status\":\"UP\",\"app\":\"benchmark-test\"}";
            exchange.getResponseHeaders().set("Content-Type", "application/json");
            exchange.sendResponseHeaders(200, response.length());
            exchange.getResponseBody().write(response.getBytes());
            exchange.close();
        });

        // Metrics info endpoint
        server.createContext("/info", exchange -> {
            MemoryMXBean memoryBean = ManagementFactory.getMemoryMXBean();
            ThreadMXBean threadBean = ManagementFactory.getThreadMXBean();

            String response = String.format(
                "{\"heap_used\":%d,\"heap_max\":%d,\"thread_count\":%d,\"peak_thread_count\":%d}",
                memoryBean.getHeapMemoryUsage().getUsed(),
                memoryBean.getHeapMemoryUsage().getMax(),
                threadBean.getThreadCount(),
                threadBean.getPeakThreadCount()
            );
            exchange.getResponseHeaders().set("Content-Type", "application/json");
            exchange.sendResponseHeaders(200, response.length());
            exchange.getResponseBody().write(response.getBytes());
            exchange.close();
        });

        server.setExecutor(null);
        server.start();
        System.out.println("HTTP server started on port 8080");
    }

    private static void startGcTrigger() {
        Thread gcThread = new Thread(() -> {
            while (true) {
                try {
                    // Allocate and release temporary memory to trigger GC
                    byte[] temp = new byte[5 * 1024 * 1024]; // 5 MB
                    temp = null;
                    Thread.sleep(5000); // Every 5 seconds
                } catch (InterruptedException e) {
                    break;
                }
            }
        }, "gc-trigger");
        gcThread.setDaemon(true);
        gcThread.start();
    }

    private static void printJvmInfo() {
        MemoryMXBean memoryBean = ManagementFactory.getMemoryMXBean();
        ThreadMXBean threadBean = ManagementFactory.getThreadMXBean();
        List<GarbageCollectorMXBean> gcBeans = ManagementFactory.getGarbageCollectorMXBeans();

        System.out.println("\nJVM Information:");
        System.out.println("  Heap Memory: " +
            formatBytes(memoryBean.getHeapMemoryUsage().getUsed()) + " / " +
            formatBytes(memoryBean.getHeapMemoryUsage().getMax()));
        System.out.println("  Non-Heap Memory: " +
            formatBytes(memoryBean.getNonHeapMemoryUsage().getUsed()));
        System.out.println("  Thread Count: " + threadBean.getThreadCount());
        System.out.println("  GC Collectors:");
        for (GarbageCollectorMXBean gc : gcBeans) {
            System.out.println("    - " + gc.getName() +
                " (collections: " + gc.getCollectionCount() +
                ", time: " + gc.getCollectionTime() + "ms)");
        }
    }

    private static String formatBytes(long bytes) {
        if (bytes < 1024) return bytes + " B";
        if (bytes < 1024 * 1024) return (bytes / 1024) + " KB";
        return (bytes / (1024 * 1024)) + " MB";
    }
}
