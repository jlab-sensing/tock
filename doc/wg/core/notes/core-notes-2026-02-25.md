# Tock Meeting Notes 2026-02-25

## Attendees
 - Amit Levy
 - Brad Campbell
 - Johnathan Van Why
 - Leon Schuermann


## Updates
 - Johnathan: Back to working on tock-registers this week. Currently have a very
   complex design that addresses all feature requests. Need to implement to see
   if it actually works, then if it does, will have to simplify design. Even if
   it works, need to see if we can stomach the complexity.

## Unmerged clippy PR #4744
 - Amit: Changes made, has not been merged because we haven't looked.
 - Johnathan: I think I found a few bugs while implementing the PR. I don't know
   if we should look now or merge it.
 - Amit: Are there new issues or existing?
 - Johnathan: Existing. There's a `static mut` buffer that the comments say the
   hardware writes to but which only immutable pointers are taken to.
 - Amit: Brad?
 - Brad: Let me take a look
 - Brad: This looks like a great compromise

## ENTRY directive PR #4745
 - Amit: We renamed everything, and exported as `_start`.
 - Leon: We've gone full circle. I thought we don't re-export, we just call it
   `initialize_and_jump_to_main` everywhere.
 - Brad: I'm agreeing with you.
 - Amit: I have no problem changing this right now. This is why we need better
   notes.
 - Amit: Oh, I see the problem. I didn't push.

## DmaSlice PR #4702
 - Amit: It has been open for a while, and I think it is quite important. I
   think we should discuss it and figure out the plan for what is outstanding
   still or whatever.
 - Leon: Plan is easy. Tock has various types of unsoundness around how we do
   DMA right now. One is we keep Rust slices to buffers that hardware is
   accessing, which produces mutable aliasing. Also, some hardware requires you
   to issue DMA-specific fence instructions to make the hardware's changes to
   the CPU. This PR introduces a new Rust type that you can convert the Rust
   buffer into, which issues the instructions needed. When the DMA operation is
   done, you run another operation that makes the hardware writes visible to the
   CPU and reconstructs the Rust buffer to return.
 - Amit: What's it still need?
 - Leon: It needs a DmaFence implementation for Cortex M chips. There only needs
   to be one implementation for all the chips because ARM defined it well. We
   also need an example of how DmaSlice is used for implementing a DMA
   peripheral platform.
 - Johnathan: Perhaps we can use the buffer that I found in my clippy PR because
   I think that is also DMA.
 - Amit: Is that reasonably easily offloadable. If I picked up and did the
   Cortex M implementation today, how likely am I to succeed?
 - Leon: Very likely. I hope the documentation is concise, precise, and covers
   everything. I've repeatedly called out for volunteers to work on this.
 - Amit: I believe you. Okay, I suppose as far as reviews go -- Johnathan,
   you've looked at this a fair amount, Brad, I think you've looked as well. Can
   we review quickly?
 - Brad: Yes
 - Johnathan: I'm happy with the core interface. I think we should have an
   expert for each architecture review that assembly.
 - Leon: Getting the core interface correct is important, the assembly is easy
   to change later.
 - Amit: The PR ports one interface that uses this. How confident are we that
   this gives enough evidence that the interface is sufficiently usable across
   different drivers. Does this PR need to include a few other examples for
   different architectures?
 - Leon: Currently low confidence, because VirtIO is the only DMA peripheral I
   can test on RISC-V, and it is a weird beast. Porting to a more "normal"
   peripheral like a UART would increase that confidence a lot. It might also
   tell us how to do it more mechanically. VirtIO required reasoning that I
   don't think applies to other uses.
 - Johnathan: Back on reviews, if we don't review the assembly now I don't think
   we'll review it unless someone hits a bug and that would suck.
 - Amit: For RISC-V it was written by Leon. For x86 we have Alex and Bobby. For
   Cortex, Pat's the expert, but he's mostly unavailable here. Brad, are you an
   expert?
 - Brad: No. Branden's an expert.
 - Amit: Yes.
 - Leon: FWIW, my main resource is reading the RISC-V documentation so I have
   moderate confidence.
 - Amit: So I think our plan of action is I will go implement this for Cortex M
   now. In doing, I will at least try to port a UART driver or something for the
   nrf. Johnathan, can you review the RISC-V?
 - Johnathan: I can, but I would have to start by looking for resources and it
   would take a while.
 - Amit: Maybe we can ask Kat or someone else.
 - Johnathan: Yes, that would be better. I can be the backup option.
