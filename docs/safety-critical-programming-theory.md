"Remove Before Flight": A Theory of Safety-Critical Software Engineering
**Introduction** Most software bugs are inconveniences. In normal applications, a crash means restart and move on. But when failure costs lives or billions of dollars, normal rules don't apply. This document presents the theory and practice of safety-critical programming through the lens of one question: *What do you remove from a language to make it safe?* ---

**The Ariane 5 Lesson: June 4, 1996**

*The Event*

At 9:33 AM local time in Kourou, French Guiana, the maiden flight of Europe's Ariane 5 rocket began. Thirty-six seconds into the flight, the rocket veered off course and self-destructed. Half a billion dollars destroyed instantly.

*The Root Cause*

A single line of code: a 64-bit floating-point value representing horizontal velocity was converted to a 16-bit integer. The value exceeded the 16-bit range. The Ada language did exactly what it was permitted to do—it raised an exception. The exception was detected but not properly handled by the flight control software. The guidance system interpreted the error as valid flight data and initiated a course correction that tore the rocket apart.

*The Deeper Lesson*

The software that failed was actually legacy code from Ariane 4. It worked perfectly on the smaller, slower rocket. On Ariane 5, higher horizontal velocities triggered an edge case nobody tested for. The code was *correct* for its original context. It was *dangerous* in its new one.

---

**The "Remove Before Flight" Philosophy**

The phrase "REMOVE BEFORE FLIGHT" is stamped on red streamers attached to protective covers on aircraft. Pilots must remove these before takeoff or risk catastrophe. The metaphor extends to software: certain language features are protective covers—useful in development, lethal in production.

*Core Principle*

Safety comes not from what you add, but from what you take away. A smaller, restricted language subset has fewer failure modes than the full language.

*The JSF C++ Standard Example*

When Lockheed Martin developed the F-35 Joint Strike Fighter, they codified this philosophy in the JSF C++ Coding Standards. Section AV Rule 208 states simply:

> **"Exceptions shall not be used."**

This is not ignorance of C++ best practices. It's a calculated tradeoff. Exception handling introduces:
- Non-local control flow that's hard to trace
- Runtime overhead from stack unwinding
- Hidden failure paths that may not be tested

By removing exceptions entirely, the F-35 team eliminated an entire category of potential failures.

---

**Theory: The Subtractive Approach to Safety**

Traditional software engineering focuses on additive features: more tests, more types, more validation. Safety-critical engineering inverts this.

*The Three Pillars*

1. **Static Over Dynamic**
   - Prefer compile-time guarantees over runtime checks
   - If a property can be proven before execution, it should be
   - Examples: Strong typing, const-correctness, array bounds checking

2. **Explicit Over Implicit**
   - No hidden control flow (exceptions, destructors with side effects)
   - No garbage collection pauses
   - Every allocation, every branch, every potential failure point must be visible in the code

3. **Fail-Safe Over Fail-Operational**
   - Assume components will fail
   - Design the failure mode to be the safest possible state
   - Ariane 5's failure was catastrophic because the error propagated rather than triggering safe mode

*Language Subsetting*

MISRA C (Motor Industry Software Reliability Association) provides perhaps the best-known example. It defines a subset of C for automotive and embedded systems. Key restrictions include:

- No dynamic memory allocation after initialization
- No recursion
- No function pointers
- All loops must have statically determinable bounds
- All code must be reachable (no "dead code")

These restrictions feel limiting until you consider what they prevent: stack overflow, memory fragmentation, untraceable call graphs, and untested code paths.

---

**Application to Financial Trading Systems**

The same principles apply when the catastrophe is financial rather than physical.

*Why Trading Systems Are Safety-Critical*

- Knight Capital lost $440 million in 45 minutes due to a software error (2012)
- A single faulty algorithm can move markets and destroy firms
- Latency requirements (microseconds) rule out garbage collection pauses
- Regulatory and reputational consequences of failures are severe

*Trading-Specific Subtractive Rules*

| Feature | Why It's Dangerous | Safer Alternative |
|---------|-------------------|-------------------|
| Dynamic allocation during trading | Latency spikes, fragmentation | Pre-allocated pools, ring buffers |
| Exceptions | Unpredictable control flow, unwinding cost | Error codes, monadic Result types |
| Reflection/RTTI | Runtime overhead, hidden complexity | Code generation, static dispatch |
| Floating-point for money | Rounding errors | Fixed-point or integer (cents) |
| Unbounded queues | Memory exhaustion | Circular buffers with backpressure |
| Hot-reloading | Untested code in production | Static binaries, blue-green deploys |

---

**Practical Implementation: The Safety-First Checklist**

When building safety-critical systems, start by answering these questions:

**Architecture**
- [ ] Have you defined a language subset with explicit allowed/denied features?
- [ ] Is there a "safe mode" that activates on any anomaly?
- [ ] Are all failure paths explicit and tested?
- [ ] Can the system operate (gracefully degrade) if any single component fails?

**Development**
- [ ] Is all code statically analyzed?
- [ ] Is there 100% branch coverage of safety-critical paths?
- [ ] Are all integer conversions explicit and range-checked?
- [ ] Is dynamic allocation prohibited during operational phases?

**Operations**
- [ ] Can you shut down any component without data loss?
- [ ] Are all state changes logged immutably?
- [ ] Is there a human in the loop for irreversible actions?
- [ ] Do you practice "remove before flight" for debug code and test flags?

---

**The Fighter Pilot Mindset**

Fighter pilots use checklists not because they can't remember procedures, but because at Mach 1, cognitive bandwidth is precious. The checklist externalizes safety, making it systematic rather than dependent on human perfection.

Programming for safety-critical systems requires the same mindset:

1. **Arrogance about failure**: Assume every line can fail
2. **Paranoia about state**: Question every assumption
3. **Humility about complexity**: If you can't reason about it completely, eliminate it
4. **Ruthlessness about features**: If it's not necessary, it's dangerous

The goal isn't perfect code. The goal is *bounded* failure—knowing exactly what can go wrong and ensuring that when it does, the system fails into the safest possible state.

---

**Conclusion**

The Ariane 5 disaster wasn't caused by a complex bug. It was caused by a simple conversion that nobody thought to check. The F-35's exception ban seems extreme until you realize it's the result of engineers asking: "What can we remove to make this safer?"

In safety-critical programming, the most important feature is the one you leave out. Remove before flight. Every time.

---

**Further Reading**

- *JSF Air Vehicle C++ Coding Standards* (Lockheed Martin, 2005)
- *MISRA C:2012 Guidelines* (MISRA Consortium)
- *The Power of 10: Rules for Developing Safety-Critical Code* (Gerard J. Holzmann, NASA)
- *DO-178C: Software Considerations in Airborne Systems* (RTCA)
- *Knight Capital Case Study: The $440 Million Software Bug*
