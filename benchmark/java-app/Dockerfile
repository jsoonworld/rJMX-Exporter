# benchmark/java-app/Dockerfile
#
# Java test application with Jolokia agent for benchmarking
# This image is used to compare rJMX-Exporter vs jmx_exporter

FROM eclipse-temurin:17-jre-alpine

# Install curl for healthcheck
RUN apk add --no-cache curl

# Create app directory
WORKDIR /app

# Download Jolokia agent
ARG JOLOKIA_VERSION=2.1.2
RUN mkdir -p /opt/jolokia && \
    curl -fsSL -o /opt/jolokia/jolokia-jvm.jar \
    "https://repo1.maven.org/maven2/org/jolokia/jolokia-agent-jvm/${JOLOKIA_VERSION}/jolokia-agent-jvm-${JOLOKIA_VERSION}-javaagent.jar"

# Copy and compile the benchmark application
COPY TestApp.java /app/
RUN apk add --no-cache openjdk17 && \
    javac TestApp.java && \
    apk del openjdk17

# Environment variables for configuration
ENV MEMORY_MB=50
ENV WORKER_THREADS=5
ENV JAVA_OPTS="-javaagent:/opt/jolokia/jolokia-jvm.jar=port=8778,host=0.0.0.0"

# Expose ports
# 8080: Application HTTP
# 8778: Jolokia JMX-over-HTTP
EXPOSE 8080 8778

# Healthcheck via Jolokia version endpoint
HEALTHCHECK --interval=10s --timeout=5s --retries=5 \
    CMD curl -f http://localhost:8778/jolokia/version || exit 1

# Run with Jolokia agent
ENTRYPOINT ["sh", "-c", "java $JAVA_OPTS -Xms${MEMORY_MB}m -Xmx256m TestApp"]
