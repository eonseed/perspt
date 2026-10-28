if (typeof Typeset === 'undefined') {
    var Typeset = {};
}

class LinkedListNode {
    constructor(data) {
        this.prev = null;
        this.next = null;
        this.data = data;
    }

    toString() {
        return this.data.toString();
    }
}

class LinkedList {
    constructor() {
        this.head = null;
        this.tail = null;
        this.listSize = 0;
    }

    *[Symbol.iterator]() {
        let node = this.head;
        while (node !== null) {
            yield node;
            node = node.next;
        }
    }

    isLinked(node) {
        if (!node || this.isEmpty()) {
            return false;
        }
        return !(node.prev === null && node.next === null && this.tail !== node && this.head !== node);
    }

    size() {
        return this.listSize;
    }

    isEmpty() {
        return this.listSize === 0;
    }

    first() {
        return this.head;
    }

    last() {
        return this.tail;
    }

    toString() {
        return this.toArray().toString();
    }

    toArray() {
        const result = [];
        for (const node of this) {
            result.push(node);
        }
        return result;
    }

    // Note that modifying the list during iteration is not safe.
    forEach(fn) {
        for (const node of this) {
            fn(node);
        }
    }

    contains(n) {
        if (!this.isLinked(n)) {
            return false;
        }
        for (const node of this) {
            if (node === n) {
                return true;
            }
        }
        return false;
    }

    at(i) {
        if (i >= this.listSize || i < 0) {
            return null;
        }

        let index = 0;
        for (const node of this) {
            if (i === index) {
                return node;
            }
            index += 1;
        }
        return null;
    }

    insertAfter(node, newNode) {
        if (!this.isLinked(node)) {
            return this;
        }
        newNode.prev = node;
        newNode.next = node.next;
        if (node.next === null) {
            this.tail = newNode;
        } else {
            node.next.prev = newNode;
        }
        node.next = newNode;
        this.listSize += 1;
        return this;
    }

    insertBefore(node, newNode) {
        if (!this.isLinked(node)) {
            return this;
        }
        newNode.prev = node.prev;
        newNode.next = node;
        if (node.prev === null) {
            this.head = newNode;
        } else {
            node.prev.next = newNode;
        }
        node.prev = newNode;
        this.listSize += 1;
        return this;
    }

    push(node) {
        if (this.head === null) {
            this.unshift(node);
        } else {
            this.insertAfter(this.tail, node);
        }
        return this;
    }

    unshift(node) {
        if (this.head === null) {
            this.head = node;
            this.tail = node;
            node.prev = null;
            node.next = null;
            this.listSize += 1;
        } else {
            this.insertBefore(this.head, node);
        }
        return this;
    }

    remove(node) {
        if (!this.isLinked(node)) {
            return this;
        }
        if (node.prev === null) {
            this.head = node.next;
        } else {
            node.prev.next = node.next;
        }
        if (node.next === null) {
            this.tail = node.prev;
        } else {
            node.next.prev = node.prev;
        }
        this.listSize -= 1;
        return this;
    }

    pop() {
        if (this.isEmpty()) {
            return null;
        }
        const node = this.tail;
        if (node.prev === null) {
            this.head = null;
            this.tail = null;
        } else {
            node.prev.next = null;
            this.tail = node.prev;
        }
        this.listSize -= 1;
        node.prev = null;
        node.next = null;
        return node;
    }

    shift() {
        if (this.isEmpty()) {
            return null;
        }
        const node = this.head;
        if (node.next === null) {
            this.head = null;
            this.tail = null;
        } else {
            node.next.prev = null;
            this.head = node.next;
        }
        this.listSize -= 1;
        node.prev = null;
        node.next = null;
        return node;
    }
}

Typeset.LinkedList = LinkedList;
Typeset.LinkedList.Node = LinkedListNode;
