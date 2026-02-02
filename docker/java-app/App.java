// docker/java-app/App.java
//
// Simple Java application that exposes JMX metrics via Jolokia

import java.lang.management.ManagementFactory;
import java.lang.management.MemoryMXBean;
import java.lang.management.ThreadMXBean;
import java.util.Random;
import com.sun.net.httpserver.HttpServer;
import java.net.InetSocketAddress;

public class App {
    private static final Random random = new Random();
    private static volatile long requestCount = 0;

    public static void main(String[] args) throws Exception {
        System.out.println("Starting sample Java application...");
        System.out.println("Jolokia endpoint: http://localhost:8778/jolokia");

        // Start a simple HTTP server
        HttpServer server = HttpServer.create(new InetSocketAddress(8080), 0);

        server.createContext("/", exchange -> {
            requestCount++;
            String response = "Hello from Java! Request #" + requestCount;
            exchange.sendResponseHeaders(200, response.length());
            exchange.getResponseBody().write(response.getBytes());
            exchange.close();
        });

        server.createContext("/health", exchange -> {
            String response = "{\"status\":\"UP\"}";
            exchange.getResponseHeaders().set("Content-Type", "application/json");
            exchange.sendResponseHeaders(200, response.length());
            exchange.getResponseBody().write(response.getBytes());
            exchange.close();
        });

        server.setExecutor(null);
        server.start();

        System.out.println("HTTP server started on port 8080");

        // Generate some load for interesting metrics
        Thread loadGenerator = new Thread(() -> {
            while (true) {
                try {
                    // Allocate some memory occasionally
                    byte[] data = new byte[random.nextInt(1024 * 100)];
                    Thread.sleep(1000);
                } catch (InterruptedException e) {
                    break;
                }
            }
        });
        loadGenerator.setDaemon(true);
        loadGenerator.start();

        // Keep the application running
        Thread.currentThread().join();
    }
}
