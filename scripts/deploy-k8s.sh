#!/bin/bash
set -e

echo "🚀 Deploying Rafka to Kubernetes..."

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    echo "❌ kubectl not found. Please install kubectl first."
    exit 1
fi

# Check if cluster is accessible
if ! kubectl cluster-info &> /dev/null; then
    echo "❌ Cannot connect to Kubernetes cluster. Please check your kubeconfig."
    exit 1
fi

echo "✅ Kubernetes cluster is accessible"

# Build Docker image
echo "🔨 Building Docker image..."
docker build -t rafka:latest .

# Load image into kind (if using kind)
if kubectl config current-context | grep -q "kind"; then
    echo "📦 Loading image into kind cluster..."
    kind load docker-image rafka:latest
fi

# Apply Kubernetes manifests
echo "📋 Applying Kubernetes manifests..."
kubectl apply -f k8s/rafka-deployment.yaml

# Wait for deployment
echo "⏳ Waiting for deployment to be ready..."
kubectl wait --for=condition=available --timeout=300s deployment/rafka-broker -n rafka

# Show status
echo "📊 Deployment status:"
kubectl get pods -n rafka
kubectl get services -n rafka

echo "✅ Rafka deployed successfully!"
echo ""
echo "🔗 Access points:"
echo "  - Broker 1: kubectl port-forward -n rafka svc/rafka-broker 50051:50051"
echo "  - Metrics: kubectl port-forward -n rafka svc/rafka-broker 9092:9092"
echo ""
echo "📝 Useful commands:"
echo "  - View logs: kubectl logs -n rafka -l app=rafka-broker"
echo "  - Scale brokers: kubectl scale -n rafka statefulset/rafka-broker --replicas=5"
echo "  - Delete deployment: kubectl delete -f k8s/rafka-deployment.yaml"
