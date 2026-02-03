/**
 * Test Java Application for rJMX-Exporter Integration Tests
 *
 * This simple application:
 * - Allocates memory to generate meaningful heap metrics
 * - Creates some threads for threading metrics
 * - Runs indefinitely to allow metric collection
 * - Triggers occasional GC for garbage collector metrics
 *
 * Used with Jolokia agent for JMX-over-HTTP access.
 */
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;

public class TestApp {

    // Hold references to prevent GC
    private static final List<byte[]> memoryHolder = new ArrayList<>();

    public static void main(String[] args) throws Exception {
        System.out.println("=== Test Java Application Started ===");
        System.out.println("Jolokia agent should be available at http://localhost:8778/jolokia");

        // Allocate some memory to generate meaningful heap metrics
        System.out.println("Allocating memory...");
        allocateMemory(10); // 10 MB

        // Create some worker threads for threading metrics
        System.out.println("Starting worker threads...");
        ExecutorService executor = Executors.newFixedThreadPool(5);
        for (int i = 0; i < 5; i++) {
            final int threadNum = i;
            executor.submit(() -> {
                Thread.currentThread().setName("Worker-" + threadNum);
                try {
                    while (!Thread.currentThread().isInterrupted()) {
                        // Simulate some work
                        Thread.sleep(1000);
                    }
                } catch (InterruptedException e) {
                    Thread.currentThread().interrupt();
                }
            });
        }

        System.out.println("Application ready for metric collection");
        System.out.println("Memory allocated: ~10 MB");
        System.out.println("Worker threads: 5");

        // Keep application running and periodically allocate/release memory
        // to generate interesting GC metrics
        int cycle = 0;
        while (true) {
            Thread.sleep(5000); // 5 seconds

            cycle++;
            if (cycle % 6 == 0) { // Every 30 seconds
                // Allocate more memory then release to trigger GC
                System.out.println("Cycle " + cycle + ": Triggering memory churn...");
                List<byte[]> temp = new ArrayList<>();
                for (int i = 0; i < 5; i++) {
                    temp.add(new byte[1024 * 1024]); // 1 MB each
                }
                temp.clear();
                System.gc(); // Suggest GC
            }

            // Print status every minute
            if (cycle % 12 == 0) {
                Runtime runtime = Runtime.getRuntime();
                long usedMemory = (runtime.totalMemory() - runtime.freeMemory()) / (1024 * 1024);
                long maxMemory = runtime.maxMemory() / (1024 * 1024);
                int threadCount = Thread.activeCount();

                System.out.printf("Status: Memory=%dMB/%dMB, Threads=%d%n",
                        usedMemory, maxMemory, threadCount);
            }
        }
    }

    /**
     * Allocate specified megabytes of memory
     */
    private static void allocateMemory(int megabytes) {
        for (int i = 0; i < megabytes; i++) {
            // Allocate 1 MB blocks
            memoryHolder.add(new byte[1024 * 1024]);
        }
    }
}
