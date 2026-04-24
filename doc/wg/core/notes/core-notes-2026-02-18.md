# Tock Meeting Notes 2026-02-18

## Attendees
 - Branden Ghena
 - Amit Levy
 - Leon Schuermann
 - Johnathan Van Why
 - Alexandru Radovici
 - Brad Campbell


## Updates
 * Branden: No Network WG update. We've been bogged down recently and haven't met
 * Amit: Crypto WG met, but I wasn't there so I'm not sure what the updates were


## Clippy Lints
 * https://github.com/tock/tock/pull/4744
 * Amit: Adds clippy lints to disallow pointer casts using the `as` keyword. Instead requires pointer casts to use explicit functions that have the same semantic meaning.
 * Amit: There's some disagreement here about whether this would be a good or bad change, so we're a little stalled.
 * Johnathan: Figuring out pointer casting has been an issue in code review for me. I even found a bug hiding in an `as` statement in a prior PR.
 * Leon: I think we do have providence-related issues in the kernel even after these changes. This helps differentiate between integers and pointers more which helps
 * Brad: From the first file in the PR change. Previously was `as *const u8` but now it's `.cast::<u8>()`. The latter seems a lot less clear to me. You have to know that cast implies pointers and not other types. It makes it look like you're casting to a u8, not a u8 pointer.
 * Johnathan: I will point at the same issue occurs with `as` casting. You have to know the type of the thing on the left to know the result.
 * Leon: Some of these casts show the types that's being cast into as a generic argument. Would it help if we always enforce that there's a type parameter passed to the cast functions.
 * Leon: The very first line has a type.
 * Brad: But even that's unclear. It's actually a pointer to a u16 pointer. Not just a u16 pointer.
 * Johnathan: I don't have a way of knowing that the left side is a pointer there at all.
 * Branden: I agree with Brad here. This code looks like it's casting to a u16* or to a u8, not to a pointer to those things. Maybe we need a function called "cast_pointer" which just calls cast for us but is named better
 * Johnathan: I was thinking an extension trait that asserts a type and lets you write a type in the source code to make things more clear.
 * Leon: I'm worried about being so different from upstream rust.
 * Amit: What's the problem you're solving Johnathan?
 * Johnathan: If the cast is between an integer and a pointer, that's got a provenance issue. That's hard to prove for soundness. Casting between pointers is fine and similar to transmute but preserves pointers. When I'm reviewing code, I need to know which is occurring. What I want is to disallow `as` casts from integers to pointers, so things are more clear.
 * Leon: So the ultimate goal is that `cast` only occurs with two pointers?
 * Johnathan: Yeah. Make it more clear where provenance issues could be.
 * Branden: I see the problem for sure, but I think this is a bad solution to it. Are there other solutions?
 * Brad: Are these the only two ways to cast in Rust?
 * Johnathan: Pointer to pointer cast? I think there are other ways that require unsafe. Like you could memcpy between two variables.
 * Branden: Is there some way to just notice just int to pointers and pointers to ints?
 * Johnathan: That would require upgrades to clippy. We'd have to push through community agreement, and then add it. Then wait for release. Or this could be some kind of Tock-specific linting upgrade.
 * Amit: This is sounding like a no. Maybe this should change into an issue so that we don't lose it.
 * Johnathan: I'm happy to put that together.
 * Alex: Could we just define a trait that uses this and has a better name? Maybe `as_pointer_to<>` with the template
 * Johnathan: I think that's doable. It would be a weird Tock-ism.
 * Branden: Depends on the pain for you Johnathan. One more weird Tock-ism is not the worst thing in the world.
 * Johnathan: Maybe. I could also have more code review comments that say "you must specify your types more clearly"
 * Brad: Looking at these, the blast radius is low. Most of these are just small impacts in chips and kernel. If these were only in the kernel trait, even if they're unclear, since it's not everywhere in Tock maybe it's fine that it's not very clear.
 * Brad: So are all of these that exist in Chips some other underlying issue that we should fix?
 * Amit: Like why are there pointer casts in chips at all? (yes)
 * Amit: Looking through: DMA one is fixable. So is the similar USB one. Vector table offset is trickier. That takes a const pointer to unit. It should arguably take something of the correct type instead. But, vector table is tricky because everything is an extern C function except for the very first element which is a pointer to the top of the stack.
 * Brad: I just brought this up because maybe Branden and I should shift our mindset to "why are we casting pointers at all". And that it's fine for those to be confusing because they should be well-commented.
 * Amit: So this clippy would be another way of saying "don't ever cast pointers"
 * Branden: I'm persuaded by the argument that there aren't very many of these in Tock so it doesn't matter a ton. But it is bonkers that the Rust community thinks this is acceptable.
 * Brad: Won't there be an unsafe block nearby?
 * Amit: Presumably to write to a register somewhere. Which may or may not be unsafe, although it probably should be.
 * Brad: Okay, I propose that we go through with this PR but first take advantage of it showing the cases where this occurs and try to either get rid of them or heavily document them. For example Process Standard is rough.
 * Johnathan: Process standard actually had a bug I found in all of this
 * Brad: I do think the underlying issue of `as` doing "everything" is a bad feature. We should move away from it.
 * Johnathan: Upstream rust agrees
 * Branden: Okay, I agree to that if we have some documentation to make code more understandable
 * Johnathan: Or change the types so it's more clear.
 * Branden: Yeah, I'm just looking for clarity. I think a comment could do it, or changing code could do it.
 * Amit: Okay, so agreement that we'll merge this once there are comments or its otherwise made clear what's going on when cast is actually used.


## ENTRY directive
 * https://github.com/tock/tock/pull/4745
 * Amit: This is my PR subsuming the PR by Eugene that adds an ENTRY directive to the linker script. This is straightforward except that the entry points should all have the same no-mangle name on every architecture and every board. Because we name that thing in the shared linker script. So what I did was rename everything to `_start` which is a conventional entry point name. Specifically on ARM, that replaces a more functionally meaningful name of `initialize_ram_and_jump_to_main` which is what that function does.
 * Amit: So the question is what we should go with?
 * Branden: Couldn't we make an `_start` function that just calls `initialize_ram_and_jump_to_main`?
 * Leon: We could, but it's got to be in assembly as code isn't initialized yet.
 * Alex: We could use "export name" to export a different name for it.
 * Leon: I think `_start` is the de facto name for exactly this purpose across all projects. So if we need to have one symbol name it should probably be clear. Naming it something more explicit doesn't really help since it doesn't allow us to change behavior across boards.
 * Brad: Say I have a board and I need to change something in `_start`? What are my options? We don't have a good story for that right now as you'd have to, for Cortex-M, change the vector table. I guess you'd need a custom chip.
 * Leon: Yeah. RISC-V has much more heterogeneity in what you need to do to boot.
 * Brad: I was thinking that if we were changing it anyways, could we make it better for people who wanted to change things. Like if you wanted to use a timer to see how long it takes to boot.
 * Amit: I think we just don't have a story for that.
 * Brad: I support Branden's proposal. Have a function called `_start` that does whatever we want, mostly just jump to `initiailize_ram_and_jump_to_main`
 * Amit: In the architecture trait, is where it would go. We don't want to write that once per chip.
 * Leon: We could have a `_start` in the chip crate, that then calls the publicly exposed arch function.
 * Brad: I guess my concern is that none of this is checked, just convention and diligent programming. Moving `_start` to the chip where we can have the chip determine what `_start` actually does seems better than having things decided in a linker script
 * Branden: Why couldn't that go into the arch crate still?
 * Amit: You're saying if we want to modify start. After this PR we'd have an ELF binary which is fine, but it has an entry symbol pointing to a function that isn't our actual start symbol.
 * Brad: Right. So the whole argument that this is convention starts to work against a reader.
 * Leon: This sounds like the cortex-m variant problem: we have a bunch of common implementations that you should use, but you can override on a per-chip basis if you need. If we're willing to accept 32-64 bytes for a structure in the source code, we could have the generic start call whatever per-chip start you want.
 * Leon: So we'd define start in the chip, and have that be a naked function that does a jump to the actual start function.
 * Amit: Is there a way to alias something in Rust so this would be free?
 * Leon: It would need to be a symbol alias. I'm not aware of a way.
 * Amit: It's just a function pointer right? A variable with no-mangle?
 * Leon: Then the static variable would have the label, not the contents of the variable
 * Leon: One very simple free solution would be a macro. Which I'm only half suggesting. That would technically meet our goals, but it doesn't really make things significantly more clear about what's happening.
 * Amit: What would this look like on RISC-V or x86? On ARM, we can define the vector table so that's easy. Those other platforms have section names, right? We'd remove the section name from the arch function, add a new symbol in each chip that's the only thing in that section.
 * Leon: And then the entry point would still use that attribute. Feels brittle though
 * Leon: What I thought was Eugene's initial goal was that we wanted to make things less brittle and assert that the start symbol is placed at the right address
 * Amit: I think so too. Right now we just assume it's the very first thing in text. And I suspect that if you have a bootloader that loads an ELF, you could have the bootloader be the entry. So we'd want to ENTRY symbol to let the bootloader know where to start the ELF.
 * Leon: We should support that use case, and also fix that what we're doing is super brittle.
 * Amit: We do want the first. Whether we can fix the brittleness could be a separate issue. The current brittleness is entirely encapsulated in the arch crate, which is something rarely touched or modified. It could be the right solution, but I'm not sure. I claim that this PR makes that problem no worse.
 * Amit: So, I think the decision for this PR ought to be to use `_start` everywhere, use some other name everywhere, or add some external name directive called `_start`.
 * Brad: To me, the rename in the linker world is the least invasive choice. It doesn't give flexibility while retaining clarity, but that seems least offensive
 * Leon: I'm very much against the symbol rename, either in Rust attributes or the linker script. I think it adds mental overhead while debugging that the symbol in the ELF symbol table doesn't match the symbol in the source code.
 * Amit: You're saying that if you objdump it would be named `_start`, and not match the source.
 * Brad: I still prefer that if we want to have an entry called start, lets just make a function called `_start`. That seems like the best option. But if we just want to meet this common name objective, then having the remapping is the easiest. What I don't care for is defining `_start` have all this initialization code which we might not want to start with. That's naming based on when they run rather than what they do.
 * Amit: I don't want to make an indirect jump function in each chip crate though. That moves the brittleness from a handful of arch crates to every chip.
 * Leon: I agree that this introduces brittleness in another way in the code. We have examples for where we need to repeat shared code across crates already, like the stack size macro. What about a start symbol marco that gets exported instead? So the chip can choose the function.
 * Amit: That's a solution to a problem separate from this PR altogether. There are some architectures that call this `_start`, and some arches that call it `initialize_ram_and_jump_to_main`. I could rename all of them to that functional name. That wouldn't address the issue of making entry extensible in the chip, but would solve the issue here.
 * Brad: I'm okay with that
 * Leon: I'm okay with that
 * Amit: Extensibility could still happen in a separate PR. And that could do additional renaming.


## DMA Slice
 * https://github.com/tock/tock/pull/4702
 * Amit: We didn't get to this today due to time. Call for people to review it.


