// This file implements the Node Orchestrator as per GitHub issue #33
// The orchestrator is responsible for managing and coordinating multiple nodes in a network.

const Node = require('./Node');

class NodeOrchestrator {
    constructor(numNodes) {
        this.nodes = Array.from({ length: numNodes }, () => new Node());
    }

    // Start all nodes
    startAllNodes() {
        this.nodes.forEach(node => node.start());
    }

    // Stop all nodes
    stopAllNodes() {
        this.nodes.forEach(node => node.stop());
    }

    // Coordinate a task across all nodes
    coordinateTask(task) {
        this.nodes.forEach(node => node.executeTask(task));
    }
}

module.exports = NodeOrchestrator;