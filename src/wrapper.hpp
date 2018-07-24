#include <wx/wx.h>
#include <cstdint>
#include <generated_wrapper.hpp>

// Because wxEvtHandler::Bind() is templated on both the functor type and the
// event type, we cannot use it. Instead, we use a trick similar to what
// wxPython does, in order to be able to specify event types dynamically.
// There shouldn't be much performance overhead from this, since we won't be
// able to monomorphize wxEvtHandler::Bind() calls from Rust anyways.

class Delegate : public wxEvtHandler {
    // Used by wxWidgets RTTI.
    DECLARE_ABSTRACT_CLASS(Delegate)
  public:
    typedef int64_t (*DelegateFunctionPointer)(void *, wxEvent *);
    void *m_data_pointer;
    DelegateFunctionPointer m_function_pointer;

    Delegate() : m_data_pointer(nullptr), m_function_pointer(nullptr), wxEvtHandler() {
    }

    Delegate(
        void *data_pointer,
        DelegateFunctionPointer function_pointer) 
      : wxEvtHandler(),
        m_function_pointer(function_pointer),
        m_data_pointer(data_pointer) {
    }

    /**
     * For calling manually.
     * event can be null
     */
    int64_t Call(wxEvent *event);

    /**
     * Actually called by wxEvtHandler::Connect
     */
    void Thunk(wxEvent& event);
};

IMPLEMENT_ABSTRACT_CLASS(Delegate, wxEvtHandler);

void BindDelegate(
    wxEvtHandler *eventSource,
    wxEventType eventType,
    Delegate *delegate,
    int id=wxID_ANY,
    int lastId=wxID_ANY);

void UnBindDelegate(
    wxEvtHandler *eventSource,
    wxEventType eventType,
    Delegate *delegate);

/**
 * This function (and calling wxEntry) are not thread safe, but can only be
 * called once. We can handle the thread safety on the Rust side (which we
 * need to do anyways).
 */
void setGlobalAppInitDelegate(Delegate *delegate);

/***
 * Workarounds for generated wrapper not being complete.
 * This is *mostly* due to the wxWidgets documentation being wrong.
 */
wxMenuBar* construct_wxMenuBar();
