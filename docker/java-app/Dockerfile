# docker/java-app/Dockerfile
#
# Simple Java application with Jolokia agent for testing

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

# Copy sample application
COPY App.java /app/

# Compile the sample app
RUN apk add --no-cache openjdk17 && \
    javac App.java && \
    apk del openjdk17

# Expose ports
EXPOSE 8080 8778

# Run with Jolokia agent
ENTRYPOINT ["sh", "-c", "java $JAVA_OPTS App"]
